use crate::capability::{call_capability, open_capability_stream, CapabilityStream};
use crate::{AppId, Result, WorkerSdkError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChatParams {
    pub role: String,
    pub messages: Vec<LlmMessage>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChatStreamChunk {
    pub content_delta: String,
    pub finish_reason: Option<String>,
    pub usage: Option<LlmUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChatResponse {
    pub content: String,
    pub finish_reason: Option<String>,
    pub usage: Option<LlmUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEmbeddingsParams {
    pub role: String,
    pub input: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEmbeddingsResponse {
    pub embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmImagesParams {
    pub role: String,
    pub prompt: String,
    pub size: Option<String>,
    pub n: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmImagesResponse {
    pub urls: Vec<String>,
}

pub struct LlmChatStream {
    inner: CapabilityStream,
}

pub async fn open_llm_chat_stream(
    socket_path: impl AsRef<Path>,
    app_id: &AppId,
    mut params: LlmChatParams,
) -> Result<LlmChatStream> {
    params.stream = Some(true);
    let encoded = postcard::to_stdvec(&params)?;
    let inner = open_capability_stream(socket_path, app_id, "llm", "chat", encoded).await?;
    Ok(LlmChatStream { inner })
}

pub async fn collect_llm_chat(
    socket_path: impl AsRef<Path>,
    app_id: &AppId,
    params: LlmChatParams,
) -> Result<LlmChatResponse> {
    let mut stream = open_llm_chat_stream(socket_path, app_id, params).await?;
    let mut content = String::new();
    let mut finish_reason = None;
    let mut usage = None;

    while let Some(event) = stream.next_event().await? {
        content.push_str(&event.content_delta);
        if event.finish_reason.is_some() {
            finish_reason = event.finish_reason;
        }
        if event.usage.is_some() {
            usage = event.usage;
        }
    }

    Ok(LlmChatResponse {
        content,
        finish_reason,
        usage,
    })
}

pub async fn call_llm_embeddings(
    socket_path: impl AsRef<Path>,
    app_id: &AppId,
    params: LlmEmbeddingsParams,
) -> Result<LlmEmbeddingsResponse> {
    let encoded = postcard::to_stdvec(&params)?;
    let response = call_capability(socket_path, app_id, "llm", "embeddings", encoded).await?;
    let bytes = response.result.map_err(|err| {
        WorkerSdkError::Protocol(format!(
            "llm embeddings failed: {}: {}",
            err.code, err.message
        ))
    })?;
    postcard::from_bytes(&bytes).map_err(WorkerSdkError::from)
}

pub async fn call_llm_images(
    socket_path: impl AsRef<Path>,
    app_id: &AppId,
    params: LlmImagesParams,
) -> Result<LlmImagesResponse> {
    let encoded = postcard::to_stdvec(&params)?;
    let response = call_capability(socket_path, app_id, "llm", "images", encoded).await?;
    let bytes = response.result.map_err(|err| {
        WorkerSdkError::Protocol(format!("llm images failed: {}: {}", err.code, err.message))
    })?;
    postcard::from_bytes(&bytes).map_err(WorkerSdkError::from)
}

impl LlmChatStream {
    pub async fn next_event(&mut self) -> Result<Option<LlmChatStreamChunk>> {
        let chunk = match self.inner.next_chunk().await? {
            Some(chunk) => chunk,
            None => return Ok(None),
        };

        if chunk.end && chunk.bytes.is_empty() {
            return Ok(None);
        }

        postcard::from_bytes(&chunk.bytes)
            .map(Some)
            .map_err(WorkerSdkError::from)
    }
}
