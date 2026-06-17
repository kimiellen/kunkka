use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, CapabilitiesConfig, ShellCapabilityConfig, WorkerCommand,
};
use kunkka_core::capability::permissions::{decide_shell_policy, ShellPolicyDecision};
use kunkka_core::capability::shell::parse_pipeline;
use kunkka_worker_sdk::AppId;

fn manifest_with_shell(allow: Vec<&str>, ask: Vec<&str>) -> AppManifest {
    AppManifest {
        app_id: AppId::new("notes"),
        worker: WorkerCommand {
            program: "/usr/bin/notes-worker".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: AppPermissions::default(),
        capabilities: CapabilitiesConfig {
            fs: None,
            shell: Some(ShellCapabilityConfig {
                allow: allow.into_iter().map(String::from).collect(),
                ask: ask.into_iter().map(String::from).collect(),
            }),
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[test]
fn parses_top_level_pipeline_commands() {
    let stages = parse_pipeline("rg todo src | wc -l").unwrap();
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].command, "rg");
    assert_eq!(stages[1].command, "wc");
}

#[test]
fn keeps_pipe_inside_quotes() {
    let stages = parse_pipeline("echo 'a|b' | rg a").unwrap();
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].command, "echo");
    assert_eq!(stages[1].command, "rg");
}

#[test]
fn rejects_redirects_and_and_operators() {
    assert!(parse_pipeline("rg todo > out.txt").is_err());
    assert!(parse_pipeline("rg todo && wc -l").is_err());
}

#[test]
fn shell_policy_allows_when_all_commands_are_allowed() {
    let manifest = manifest_with_shell(vec!["rg", "wc"], vec![]);
    let decision = decide_shell_policy(&manifest, &["rg".to_string(), "wc".to_string()]);
    assert_eq!(decision, ShellPolicyDecision::Allow);
}

#[test]
fn shell_policy_asks_when_any_command_requires_approval() {
    let manifest = manifest_with_shell(vec!["rg"], vec!["curl"]);
    let decision = decide_shell_policy(&manifest, &["rg".to_string(), "curl".to_string()]);
    assert_eq!(decision, ShellPolicyDecision::Ask);
}

#[test]
fn shell_policy_denies_when_command_is_unlisted() {
    let manifest = manifest_with_shell(vec!["rg"], vec!["curl"]);
    let decision = decide_shell_policy(&manifest, &["python".to_string()]);
    assert_eq!(decision, ShellPolicyDecision::Deny);
}
