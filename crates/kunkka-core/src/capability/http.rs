use crate::app_manifest::AppManifest;
use crate::capability::CapabilityError;
use serde::{Deserialize, Serialize};

const REQUEST_TIMEOUT_SECS: u64 = 30;
const MAX_REDIRECTS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestParams {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

const ALLOWED_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

pub async fn handle_http_request(
    manifest: &AppManifest,
    method: &str,
    params: &[u8],
) -> Result<Vec<u8>, CapabilityError> {
    if method != "request" {
        return Err(CapabilityError {
            code: "unknown_method".to_string(),
            message: format!("unknown http method: {method}"),
        });
    }

    let params: HttpRequestParams = postcard::from_bytes(params).map_err(|e| CapabilityError {
        code: "invalid_params".to_string(),
        message: format!("invalid params: {e}"),
    })?;

    let url = reqwest::Url::parse(&params.url).map_err(|e| CapabilityError {
        code: "invalid_params".to_string(),
        message: format!("invalid url: {e}"),
    })?;

    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(CapabilityError {
            code: "scheme_not_allowed".to_string(),
            message: format!("url scheme must be http or https, got: {}", url.scheme()),
        });
    }

    let method_upper = params.method.to_uppercase();
    if !ALLOWED_METHODS.contains(&method_upper.as_str()) {
        return Err(CapabilityError {
            code: "invalid_params".to_string(),
            message: format!(
                "http method must be one of {:?}, got: {}",
                ALLOWED_METHODS, params.method
            ),
        });
    }

    let http_config = manifest
        .capabilities
        .http
        .as_ref()
        .ok_or_else(|| CapabilityError {
            code: "permission_denied".to_string(),
            message: "app does not have http capability configured".to_string(),
        })?;

    let host = url.host_str().ok_or_else(|| CapabilityError {
        code: "invalid_params".to_string(),
        message: "url has no host".to_string(),
    })?;

    if !http_config
        .domains
        .iter()
        .any(|d| d.eq_ignore_ascii_case(host))
    {
        return Err(CapabilityError {
            code: "permission_denied".to_string(),
            message: format!("domain {host} is not in the allowed list"),
        });
    }

    let allowed_domains: Vec<String> = http_config.domains.clone();
    let redirect_policy = reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.stop();
        }
        match attempt.url().host_str() {
            Some(host) if allowed_domains.iter().any(|d| d.eq_ignore_ascii_case(host)) => {
                attempt.follow()
            }
            _ => attempt.error(std::io::Error::other("redirect to non-whitelisted domain")),
        }
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .no_proxy()
        .redirect(redirect_policy)
        .build()
        .map_err(|e| CapabilityError {
            code: "io_error".to_string(),
            message: format!("failed to create http client: {e}"),
        })?;

    let reqwest_method =
        reqwest::Method::from_bytes(method_upper.as_bytes()).map_err(|e| CapabilityError {
            code: "invalid_params".to_string(),
            message: format!("invalid method: {e}"),
        })?;

    let mut request = client.request(reqwest_method, url);
    for (key, value) in &params.headers {
        request = request.header(key.as_str(), value.as_str());
    }
    if let Some(body) = params.body {
        request = request.body(body);
    }

    let response = request.send().await.map_err(|e| {
        if e.is_timeout() {
            CapabilityError {
                code: "timeout".to_string(),
                message: "request timed out".to_string(),
            }
        } else {
            CapabilityError {
                code: "io_error".to_string(),
                message: format!("http request failed: {e}"),
            }
        }
    })?;

    let status_code = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = response.bytes().await.map_err(|e| CapabilityError {
        code: "io_error".to_string(),
        message: format!("failed to read response body: {e}"),
    })?;

    let http_response = HttpResponse {
        status_code,
        headers,
        body: body.to_vec(),
    };

    postcard::to_stdvec(&http_response).map_err(|e| CapabilityError {
        code: "io_error".to_string(),
        message: format!("encode result: {e}"),
    })
}
