use kunkka_core::app_manifest::{AppManifest, CapabilitiesConfig, HttpCapabilityConfig};
use kunkka_core::capability::http::{handle_http_request, HttpRequestParams};

fn make_manifest(domains: &[&str]) -> AppManifest {
    AppManifest {
        app_id: kunkka_worker_sdk::AppId::new("test"),
        worker: kunkka_core::app_manifest::WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: Default::default(),
        capabilities: CapabilitiesConfig {
            fs: None,
            shell: None,
            http: Some(HttpCapabilityConfig {
                domains: domains.iter().map(|s| s.to_string()).collect(),
            }),
            llm: None,
        },
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    }
}

fn encode_params(params: &HttpRequestParams) -> Vec<u8> {
    postcard::to_stdvec(params).unwrap()
}

#[tokio::test]
async fn test_unknown_method() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "unknown", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "unknown_method");
}

#[tokio::test]
async fn test_invalid_url() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "not a url".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "invalid_params");
}

#[tokio::test]
async fn test_scheme_not_allowed() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "ftp://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "scheme_not_allowed");
}

#[tokio::test]
async fn test_domain_not_in_whitelist() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://evil.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
}

#[tokio::test]
async fn test_domain_case_insensitive() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://API.GITHUB.COM".to_string(),
        headers: vec![],
        body: None,
    });
    // This will fail with io_error because the server doesn't exist,
    // but it should NOT fail with permission_denied
    let result = handle_http_request(&manifest, "request", &params).await;
    if let Err(err) = result {
        assert_ne!(err.code, "permission_denied");
    }
}

#[tokio::test]
async fn test_invalid_http_method() {
    let manifest = make_manifest(&["api.github.com"]);
    let params = encode_params(&HttpRequestParams {
        method: "TRACE".to_string(),
        url: "https://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "invalid_params");
}

#[tokio::test]
async fn test_no_http_capability_config() {
    let manifest = AppManifest {
        app_id: kunkka_worker_sdk::AppId::new("test"),
        worker: kunkka_core::app_manifest::WorkerCommand {
            program: "/usr/bin/test".to_string(),
            args: vec![],
            env: Default::default(),
            cwd: None,
        },
        permissions: Default::default(),
        capabilities: CapabilitiesConfig::default(),
        idle_timeout_ms: 300_000,
        startup_timeout_ms: 10_000,
    };
    let params = encode_params(&HttpRequestParams {
        method: "GET".to_string(),
        url: "https://api.github.com".to_string(),
        headers: vec![],
        body: None,
    });
    let result = handle_http_request(&manifest, "request", &params).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "permission_denied");
}
