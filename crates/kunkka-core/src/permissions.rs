use crate::app_manifest::AppManifest;

pub enum PermissionDecision {
    Allow,
    Deny { code: &'static str, message: String },
}

pub fn decide_frontend_dispatch(manifest: &AppManifest, method: &str) -> PermissionDecision {
    if manifest
        .permissions
        .frontend_dispatch
        .allowed_methods
        .iter()
        .any(|allowed| allowed == method)
    {
        PermissionDecision::Allow
    } else {
        PermissionDecision::Deny {
            code: "permission_denied",
            message: format!(
                "frontend dispatch method {:?} is not allowed for app {:?}",
                method,
                manifest.app_id.as_str()
            ),
        }
    }
}
