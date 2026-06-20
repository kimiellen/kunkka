# LLM 能力 (LLM Capability)

## 概述

LLM 能力允许 Worker 通过 Core 代理访问 LLM（大语言模型）服务。Worker 通过角色名调用 LLM，Core 根据角色配置路由到对应的供应商和模型。

## 协议

- Schema: `kunkka.capability.v1`
- Capability: `llm`

## 方法

### chat

发起 Chat 对话请求（非流式）。

**请求参数：**

```rust
struct LlmChatParams {
    role: String,                   // 角色名
    messages: Vec<LlmMessage>,      // 消息列表
    stream: Option<bool>,           // 是否流式（false 或省略）
    temperature: Option<f32>,       // 温度参数
    max_tokens: Option<u32>,        // 最大 token 数
}

struct LlmMessage {
    role: String,    // "system" 或 "user"
    content: String, // 消息内容
}
```

**响应结果：**

```rust
enum LlmResponse {
    Chat(LlmChatResponse),
}

struct LlmChatResponse {
    content: String,                // 生成的内容
    finish_reason: Option<String>,  // 结束原因
    usage: Option<LlmUsage>,       // token 使用量
}

struct LlmUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}
```

### chat（流式）

发起流式 Chat 对话请求。

**请求参数：** 同上，但 `stream` 设为 `true`

**响应：** 通过 Stream 帧逐 chunk 返回

```rust
struct LlmChatStreamChunk {
    content_delta: String,          // 本次增量文本
    finish_reason: Option<String>,  // 结束原因（最后一个 chunk）
    usage: Option<LlmUsage>,       // token 使用量（最后一个 chunk）
}
```

### embeddings

将文本转换为向量表示。

**请求参数：**

```rust
struct LlmEmbeddingsParams {
    role: String,           // 角色名
    input: Vec<String>,     // 要向量化的文本列表
}
```

**响应结果：**

```rust
enum LlmResponse {
    Embeddings(LlmEmbeddingsResponse),
}

struct LlmEmbeddingsResponse {
    embeddings: Vec<Vec<f32>>,  // 向量列表
}
```

### images

根据文字描述生成图片。

**请求参数：**

```rust
struct LlmImagesParams {
    role: String,           // 角色名
    prompt: String,         // 图片描述
    size: Option<String>,   // 图片尺寸（"256x256"、"512x512"、"1024x1024" 等）
    n: Option<u32>,         // 生成数量
}
```

**响应结果：**

```rust
enum LlmResponse {
    Images(LlmImagesResponse),
}

struct LlmImagesResponse {
    urls: Vec<String>,  // 生成的图片 URL 列表
}
```

### list_providers

列出所有已配置的 LLM 供应商。

**响应：**

```rust
enum LlmResponse {
    Providers(Vec<String>),  // 供应商名称列表
}
```

### list_models

列出所有可用的模型。

**响应：**

```rust
enum LlmResponse {
    Models(Vec<(String, String)>),  // (供应商名, 模型名) 列表
}
```

### list_roles

列出所有已配置的角色。

**响应：**

```rust
enum LlmResponse {
    Roles(Vec<String>),  // 角色名称列表
}
```

## 配置文件

### 供应商配置

`~/.config/kunkka/llm-providers.json`：

```json
{
  "providers": {
    "openai": {
      "provider_type": "api_key",
      "base_url": "https://api.openai.com/v1",
      "api_key": "sk-...",
      "available_models": ["gpt-4o", "gpt-4o-mini"]
    },
    "ollama": {
      "provider_type": "local",
      "base_url": "http://localhost:11434/v1",
      "api_key": "ollama",
      "available_models": ["llama2", "mistral"]
    }
  }
}
```

### 角色配置

`~/.config/kunkka/llm-roles.json`：

```json
{
  "roles": {
    "thinker": {
      "description": "深度思考，用于复杂推理",
      "provider": "openai",
      "model": "gpt-4o",
      "parameters": {
        "temperature": 0.7,
        "max_tokens": 4096
      }
    },
    "coder": {
      "description": "代码生成，更精确",
      "provider": "openai",
      "model": "gpt-4o",
      "parameters": {
        "temperature": 0.3,
        "max_tokens": 8192
      }
    }
  }
}
```

## Worker SDK 使用示例

### 非流式 Chat

```rust
use kunkka_worker_sdk::{
    collect_llm_chat, AppId, LlmChatParams, LlmMessage, LlmChatResponse,
};

async fn simple_chat(
    socket_path: &Path,
    app_id: &AppId,
) -> Result<LlmChatResponse, Box<dyn std::error::Error>> {
    let params = LlmChatParams {
        role: "thinker".to_string(),
        messages: vec![
            LlmMessage {
                role: "user".to_string(),
                content: "什么是量子计算？".to_string(),
            },
        ],
        stream: None,
        temperature: Some(0.7),
        max_tokens: Some(1024),
    };

    let response = collect_llm_chat(socket_path, app_id, params).await?;
    Ok(response)
}
```

### 流式 Chat

```rust
use kunkka_worker_sdk::{open_llm_chat_stream, AppId, LlmChatParams, LlmMessage};

async fn stream_chat(
    socket_path: &Path,
    app_id: &AppId,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = LlmChatParams {
        role: "thinker".to_string(),
        messages: vec![
            LlmMessage {
                role: "user".to_string(),
                content: "解释什么是量子计算".to_string(),
            },
        ],
        stream: None,  // helper 自动设为 true
        temperature: Some(0.7),
        max_tokens: Some(1024),
    };

    let mut stream = open_llm_chat_stream(socket_path, app_id, params).await?;

    while let Some(chunk) = stream.next_event().await? {
        print!("{}", chunk.content_delta);
    }

    println!();
    Ok(())
}
```

### Embeddings

```rust
use kunkka_worker_sdk::{call_llm_embeddings, AppId, LlmEmbeddingsParams};

async fn get_embeddings(
    socket_path: &Path,
    app_id: &AppId,
) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    let params = LlmEmbeddingsParams {
        role: "collector".to_string(),
        input: vec![
            "什么是机器学习".to_string(),
            "人工智能的发展历史".to_string(),
        ],
    };

    let response = call_llm_embeddings(socket_path, app_id, params).await?;
    Ok(response.embeddings)
}
```

## 角色系统

角色是 LLM 调用的抽象层，每个角色定义了：

- 使用哪个供应商
- 使用哪个模型
- 默认参数（temperature、max_tokens 等）

Worker 通过角色名调用 LLM，无需关心底层供应商和模型配置。

### 预设角色

Kunkka 提供了一些预设角色，可以通过 `apply_role_preset` 快速配置。

## 使用统计

LLM 能力会自动记录每次调用的 token 使用量，可以通过以下方法查询：

- `usage_summary`：获取汇总统计
- `usage_records`：获取最近的调用记录
- `clear_usage`：清空使用记录

## 安全注意事项

- API Key 存储在 Core 的配置文件中，Worker 无法直接访问
- 所有 LLM 请求都通过 Core 代理，Worker 不直接连接 LLM 服务
- 使用统计帮助监控 API 调用量
