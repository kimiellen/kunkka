# Worker 侧 LLM Capability 使用指南

## 概述

`kunkka-worker-sdk` 提供了 typed helper，让 Worker 可以通过 Core 代理访问 LLM 能力，无需直接管理 API key 或处理 IPC 协议细节。

Worker 通过角色名调用 LLM，Core 根据角色配置路由到对应的供应商和模型。

## 前置条件

1. Core 已启动并运行
2. LLM 供应商已配置（`~/.config/kunkka/llm-providers.json`）
3. 角色已定义（`~/.config/kunkka/llm-roles.json`）

## API 概览

| 接口 | 用途 | 返回方式 |
|------|------|----------|
| `open_llm_chat_stream()` | 流式 Chat | `LlmChatStream` 逐 chunk 读取 |
| `collect_llm_chat()` | 非流式 Chat | `LlmChatResponse` 完整返回 |
| `call_llm_embeddings()` | 向量化 | `LlmEmbeddingsResponse` |
| `call_llm_images()` | 图片生成 | `LlmImagesResponse` |

## 使用示例

### 1. 流式 Chat

实时接收生成内容，适合需要逐步输出的场景：

```rust
use kunkka_worker_sdk::{
    open_llm_chat_stream, AppId, LlmChatParams, LlmMessage,
};
use std::path::Path;

async fn stream_chat_example(socket_path: &Path, app_id: &AppId) -> Result<(), Box<dyn std::error::Error>> {
    let params = LlmChatParams {
        role: "thinker".to_string(),
        messages: vec![
            LlmMessage {
                role: "system".to_string(),
                content: "你是一个有用的助手".to_string(),
            },
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
        // chunk.content_delta: 本次增量文本
        // chunk.finish_reason: 生成结束原因（如有）
        // chunk.usage: token 使用量（通常在最后一个 chunk）
        print!("{}", chunk.content_delta);
    }

    println!(); // 换行
    Ok(())
}
```

### 2. 非流式 Chat

一次性获取完整响应，适合简单的请求-响应场景：

```rust
use kunkka_worker_sdk::{
    collect_llm_chat, AppId, LlmChatParams, LlmMessage, LlmChatResponse,
};
use std::path::Path;

async fn simple_chat_example(socket_path: &Path, app_id: &AppId) -> Result<(), Box<dyn std::error::Error>> {
    let params = LlmChatParams {
        role: "coder".to_string(),
        messages: vec![
            LlmMessage {
                role: "user".to_string(),
                content: "写一个 Rust 函数计算斐波那契数列".to_string(),
            },
        ],
        stream: None,
        temperature: Some(0.3),  // 编码任务用较低温度
        max_tokens: Some(2048),
    };

    let response: LlmChatResponse = collect_llm_chat(socket_path, app_id, params).await?;

    println!("生成内容：{}", response.content);

    if let Some(finish) = &response.finish_reason {
        println!("结束原因：{}", finish);
    }

    if let Some(usage) = &response.usage {
        println!("Token 用量：prompt={}, completion={}, total={}",
            usage.prompt_tokens, usage.completion_tokens, usage.total_tokens);
    }

    Ok(())
}
```

### 3. Embeddings

将文本转换为向量表示，用于语义搜索、相似度计算等：

```rust
use kunkka_worker_sdk::{
    call_llm_embeddings, AppId, LlmEmbeddingsParams,
};
use std::path::Path;

async fn embeddings_example(socket_path: &Path, app_id: &AppId) -> Result<(), Box<dyn std::error::Error>> {
    let params = LlmEmbeddingsParams {
        role: "collector".to_string(),
        input: vec![
            "什么是机器学习".to_string(),
            "人工智能的发展历史".to_string(),
            "深度学习与神经网络".to_string(),
        ],
    };

    let response = call_llm_embeddings(socket_path, app_id, params).await?;

    for (i, embedding) in response.embeddings.iter().enumerate() {
        println!("文本 {} 的向量维度：{}", i, embedding.len());
        println!("前 5 维：{:?}", &embedding[..5.min(embedding.len())]);
    }

    Ok(())
}
```

### 4. 图片生成

根据文字描述生成图片：

```rust
use kunkka_worker_sdk::{
    call_llm_images, AppId, LlmImagesParams,
};
use std::path::Path;

async fn images_example(socket_path: &Path, app_id: &AppId) -> Result<(), Box<dyn std::error::Error>> {
    let params = LlmImagesParams {
        role: "designer".to_string(),
        prompt: "一只可爱的橘猫坐在书桌上，旁边有一杯咖啡".to_string(),
        size: Some("1024x1024".to_string()),
        n: Some(1),
    };

    let response = call_llm_images(socket_path, app_id, params).await?;

    for url in &response.urls {
        println!("生成的图片：{}", url);
    }

    Ok(())
}
```

### 5. 多轮对话

维护对话上下文进行多轮交互：

```rust
use kunkka_worker_sdk::{
    collect_llm_chat, AppId, LlmChatParams, LlmMessage,
};
use std::path::Path;

async fn multi_turn_chat(socket_path: &Path, app_id: &AppId) -> Result<(), Box<dyn std::error::Error>> {
    let mut messages = vec![
        LlmMessage {
            role: "system".to_string(),
            content: "你是一个 Rust 编程助手".to_string(),
        },
    ];

    // 第一轮
    messages.push(LlmMessage {
        role: "user".to_string(),
        content: "什么是所有权系统？".to_string(),
    });

    let response1 = collect_llm_chat(
        socket_path,
        app_id,
        LlmChatParams {
            role: "thinker".to_string(),
            messages: messages.clone(),
            stream: None,
            temperature: Some(0.7),
            max_tokens: Some(512),
        },
    ).await?;

    // 将助手回复加入上下文
    messages.push(LlmMessage {
        role: "assistant".to_string(),
        content: response1.content.clone(),
    });

    // 第二轮（引用之前的回答）
    messages.push(LlmMessage {
        role: "user".to_string(),
        content: "能给一个具体的代码示例吗？".to_string(),
    });

    let response2 = collect_llm_chat(
        socket_path,
        app_id,
        LlmChatParams {
            role: "thinker".to_string(),
            messages,
            stream: None,
            temperature: Some(0.7),
            max_tokens: Some(1024),
        },
    ).await?;

    println!("回答：{}", response2.content);
    Ok(())
}
```

## 错误处理

所有 helper 返回 `Result<T, WorkerSdkError>`，常见错误：

```rust
use kunkka_worker_sdk::WorkerSdkError;

match collect_llm_chat(socket_path, app_id, params).await {
    Ok(response) => { /* 成功 */ }
    Err(WorkerSdkError::Protocol(msg)) => {
        // Core 返回错误（如角色不存在、供应商不可用、LLM 请求失败）
        eprintln!("LLM 调用失败：{}", msg);
    }
    Err(WorkerSdkError::Ipc(err)) => {
        // IPC 连接错误（Core 未启动、socket 不存在）
        eprintln!("IPC 错误：{}", err);
    }
    Err(WorkerSdkError::Codec(err)) => {
        // 编解码错误
        eprintln!("编码错误：{}", err);
    }
}
```

## 角色配置示例

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
    },
    "collector": {
      "description": "信息收集，快速响应",
      "provider": "openai",
      "model": "gpt-4o-mini",
      "parameters": {
        "temperature": 0.5,
        "max_tokens": 2048
      }
    },
    "designer": {
      "description": "图片生成",
      "provider": "openai",
      "model": "dall-e-3",
      "parameters": {}
    }
  }
}
```

## 供应商配置示例

`~/.config/kunkka/llm-providers.json`：

```json
{
  "providers": {
    "openai": {
      "provider_type": "api_key",
      "base_url": "https://api.openai.com/v1",
      "api_key": "sk-...",
      "available_models": ["gpt-4o", "gpt-4o-mini", "dall-e-3"]
    },
    "claude": {
      "provider_type": "api_key",
      "base_url": "https://api.anthropic.com",
      "api_key": "sk-ant-...",
      "available_models": ["claude-sonnet-4-20250514"]
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

## 底层接口

如果需要更底层的控制，可以直接使用 `call_capability`：

```rust
use kunkka_worker_sdk::call_capability;

let params = /* postcard 编码的请求参数 */;
let response = call_capability(socket_path, app_id, "llm", "chat", params).await?;
```

但推荐使用 typed helper，它们自动处理编码解码和错误映射。
