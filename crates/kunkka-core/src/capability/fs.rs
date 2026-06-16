use crate::app_manifest::AppManifest;
use crate::capability::permissions::check_fs_permission;
use crate::capability::CapabilityError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileParams {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadFileResult {
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileParams {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileResult {
    pub bytes_written: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListDirParams {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListDirResult {
    pub entries: Vec<DirEntry>,
}

fn decode_params<T: serde::de::DeserializeOwned>(params: &[u8]) -> Result<T, CapabilityError> {
    postcard::from_bytes(params).map_err(|e| CapabilityError {
        code: "io_error".to_string(),
        message: format!("invalid params: {e}"),
    })
}

pub async fn handle_fs_request(
    manifest: &AppManifest,
    method: &str,
    params: &[u8],
) -> Result<Vec<u8>, CapabilityError> {
    match method {
        "read_file" => {
            let p: ReadFileParams = decode_params(params)?;
            check_fs_permission(manifest, &p.path)?;
            let content = tokio::fs::read_to_string(&p.path).await.map_err(|e| {
                let code = match e.kind() {
                    std::io::ErrorKind::NotFound => "not_found",
                    std::io::ErrorKind::PermissionDenied => "permission_denied",
                    std::io::ErrorKind::InvalidData => "not_utf8",
                    _ => "io_error",
                };
                CapabilityError {
                    code: code.to_string(),
                    message: e.to_string(),
                }
            })?;
            postcard::to_stdvec(&ReadFileResult { content }).map_err(|e| CapabilityError {
                code: "io_error".to_string(),
                message: format!("encode result: {e}"),
            })
        }
        "write_file" => {
            let p: WriteFileParams = decode_params(params)?;
            check_fs_permission(manifest, &p.path)?;
            tokio::fs::write(&p.path, &p.content).await.map_err(|e| {
                let code = match e.kind() {
                    std::io::ErrorKind::PermissionDenied => "permission_denied",
                    _ => "io_error",
                };
                CapabilityError {
                    code: code.to_string(),
                    message: e.to_string(),
                }
            })?;
            let bytes_written = p.content.len() as u64;
            postcard::to_stdvec(&WriteFileResult { bytes_written }).map_err(|e| CapabilityError {
                code: "io_error".to_string(),
                message: format!("encode result: {e}"),
            })
        }
        "list_dir" => {
            let p: ListDirParams = decode_params(params)?;
            check_fs_permission(manifest, &p.path)?;
            let mut entries = Vec::new();
            let mut dir = tokio::fs::read_dir(&p.path).await.map_err(|e| {
                let code = match e.kind() {
                    std::io::ErrorKind::NotFound => "not_found",
                    std::io::ErrorKind::PermissionDenied => "permission_denied",
                    _ => "io_error",
                };
                CapabilityError {
                    code: code.to_string(),
                    message: e.to_string(),
                }
            })?;
            while let Some(entry) = dir.next_entry().await.map_err(|e| CapabilityError {
                code: "io_error".to_string(),
                message: e.to_string(),
            })? {
                let file_type = entry.file_type().await.map_err(|e| CapabilityError {
                    code: "io_error".to_string(),
                    message: e.to_string(),
                })?;
                let metadata = entry.metadata().await.map_err(|e| CapabilityError {
                    code: "io_error".to_string(),
                    message: e.to_string(),
                })?;
                let entry_type = if file_type.is_dir() {
                    "dir"
                } else if file_type.is_file() {
                    "file"
                } else if file_type.is_symlink() {
                    "symlink"
                } else {
                    "other"
                };
                entries.push(DirEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    entry_type: entry_type.to_string(),
                    size: metadata.len(),
                });
            }
            postcard::to_stdvec(&ListDirResult { entries }).map_err(|e| CapabilityError {
                code: "io_error".to_string(),
                message: format!("encode result: {e}"),
            })
        }
        _ => Err(CapabilityError {
            code: "unknown_method".to_string(),
            message: format!("unknown fs method: {method}"),
        }),
    }
}
