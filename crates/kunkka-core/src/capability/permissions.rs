use crate::app_manifest::AppManifest;
use crate::capability::CapabilityError;
use std::path::{Component, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellPolicyDecision {
    Allow,
    Ask,
    Deny,
}

pub fn check_fs_permission(manifest: &AppManifest, path: &str) -> Result<(), CapabilityError> {
    let fs_config = manifest
        .capabilities
        .fs
        .as_ref()
        .ok_or_else(|| CapabilityError {
            code: "permission_denied".to_string(),
            message: "app has no fs capability configured".to_string(),
        })?;

    if fs_config.paths.is_empty() {
        return Err(CapabilityError {
            code: "permission_denied".to_string(),
            message: "app fs capability has no allowed paths".to_string(),
        });
    }

    let normalized = normalize_path(path);

    for allowed in &fs_config.paths {
        if allowed.ends_with('/') {
            let prefix = normalize_path(allowed);
            let trimmed = prefix.trim_end_matches('/');
            if normalized.starts_with(trimmed)
                && (normalized.len() == trimmed.len()
                    || normalized.as_bytes().get(trimmed.len()) == Some(&b'/'))
            {
                return Ok(());
            }
        } else {
            let exact = normalize_path(allowed);
            if normalized == exact {
                return Ok(());
            }
        }
    }

    Err(CapabilityError {
        code: "permission_denied".to_string(),
        message: format!(
            "path {:?} is not in allowed fs paths for app {:?}",
            path, manifest.app_id
        ),
    })
}

pub fn decide_shell_policy(manifest: &AppManifest, commands: &[String]) -> ShellPolicyDecision {
    let Some(shell_config) = manifest.capabilities.shell.as_ref() else {
        return ShellPolicyDecision::Deny;
    };

    let mut saw_ask = false;

    for command in commands {
        if shell_config.allow.iter().any(|allowed| allowed == command) {
            continue;
        }

        if shell_config.ask.iter().any(|ask| ask == command) {
            saw_ask = true;
            continue;
        }

        return ShellPolicyDecision::Deny;
    }

    if saw_ask {
        ShellPolicyDecision::Ask
    } else {
        ShellPolicyDecision::Allow
    }
}

fn normalize_path(path: &str) -> String {
    let path = PathBuf::from(path);
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(c) => components.push(c.to_string_lossy().into_owned()),
            Component::RootDir => components.push(String::new()),
            Component::ParentDir => {
                if components.len() > 1 {
                    components.pop();
                }
            }
            Component::CurDir => {}
            _ => {}
        }
    }
    let result = components.join("/");
    if result.is_empty() {
        "/".to_string()
    } else if path.to_string_lossy().ends_with('/') && !result.ends_with('/') {
        format!("{result}/")
    } else {
        result
    }
}
