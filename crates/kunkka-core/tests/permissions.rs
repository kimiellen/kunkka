use kunkka_core::app_manifest::{
    AppManifest, AppPermissions, FrontendDispatchPermissions, WorkerCommand,
};
use kunkka_core::permissions::{decide_frontend_dispatch, PermissionDecision};
use kunkka_worker_sdk::AppId;
use std::collections::BTreeMap;

fn manifest_with_methods(methods: &[&str]) -> AppManifest {
    AppManifest {
        app_id: AppId::new("test-app"),
        worker: WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            cwd: None,
        },
        permissions: AppPermissions {
            frontend_dispatch: FrontendDispatchPermissions {
                allowed_methods: methods.iter().map(|s| s.to_string()).collect(),
            },
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

#[test]
fn allows_method_present_in_allowed_methods() {
    let manifest = manifest_with_methods(&["search", "open"]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, "search"),
        PermissionDecision::Allow
    ));
}

#[test]
fn denies_method_not_in_allowed_methods() {
    let manifest = manifest_with_methods(&["search"]);
    let decision = decide_frontend_dispatch(&manifest, "delete");
    assert!(matches!(
        decision,
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}

#[test]
fn denies_when_allowed_methods_is_empty() {
    let manifest = manifest_with_methods(&[]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, "search"),
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}

#[test]
fn method_matching_is_case_sensitive() {
    let manifest = manifest_with_methods(&["Search"]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, "search"),
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}

#[test]
fn method_matching_does_not_trim() {
    let manifest = manifest_with_methods(&["search"]);
    assert!(matches!(
        decide_frontend_dispatch(&manifest, " search"),
        PermissionDecision::Deny { code: "permission_denied", .. }
    ));
}
