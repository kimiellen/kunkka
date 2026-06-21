use kunkka_ipc::{EndpointId, Frame, FrameMetadata, IpcListener, Payload, SessionId, StreamId};
use kunkka_worker_sdk::capability::{
    decode_capability_request, encode_capability_response, CapabilityResponse,
};
use kunkka_worker_sdk::llm::{
    call_llm_embeddings, call_llm_images, collect_llm_chat, open_llm_chat_stream, LlmChatParams,
    LlmChatResponse, LlmChatStreamChunk, LlmEmbeddingsParams, LlmEmbeddingsResponse,
    LlmImagesParams, LlmImagesResponse, LlmMessage, LlmUsage,
};
use kunkka_worker_sdk::AppId;
use tempfile::{tempdir, TempDir};

fn socket_path() -> (TempDir, std::path::PathBuf) {
    let root = tempdir().unwrap();
    let path = root.path().join("llm-stream.sock");
    (root, path)
}

#[tokio::test]
async fn open_llm_chat_stream_decodes_typed_chunks() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();
        let Frame::Request {
            request_id,
            session_id,
            payload,
            ..
        } = frame
        else {
            panic!("expected request frame");
        };

        let request = decode_capability_request(&payload).unwrap();
        assert_eq!(request.app_id, "notes");
        assert_eq!(request.capability, "llm");
        assert_eq!(request.method, "chat");

        let params: LlmChatParams = postcard::from_bytes(&request.params).unwrap();
        assert_eq!(params.role, "thinker");
        assert_eq!(params.stream, Some(true));

        let first = LlmChatStreamChunk {
            content_delta: "hel".to_string(),
            finish_reason: None,
            usage: None,
        };
        let second = LlmChatStreamChunk {
            content_delta: "lo".to_string(),
            finish_reason: Some("stop".to_string()),
            usage: Some(LlmUsage {
                prompt_tokens: 3,
                completion_tokens: 2,
                total_tokens: 5,
            }),
        };

        for chunk in [first, second] {
            connection
                .send_frame(&Frame::Stream {
                    stream_id: StreamId(7),
                    request_id: Some(request_id),
                    session_id,
                    source: EndpointId::new("core"),
                    target: EndpointId::new("worker-sdk"),
                    payload: Payload {
                        bytes: postcard::to_stdvec(&chunk).unwrap(),
                        content_type: None,
                        schema: None,
                        metadata: FrameMetadata::new(),
                    },
                    end: false,
                    metadata: FrameMetadata::new(),
                })
                .await
                .unwrap();
        }

        connection
            .send_frame(&Frame::Stream {
                stream_id: StreamId(7),
                request_id: Some(request_id),
                session_id,
                source: EndpointId::new("core"),
                target: EndpointId::new("worker-sdk"),
                payload: Payload {
                    bytes: Vec::new(),
                    content_type: None,
                    schema: None,
                    metadata: FrameMetadata::new(),
                },
                end: true,
                metadata: FrameMetadata::new(),
            })
            .await
            .unwrap();
    });

    let params = LlmChatParams {
        role: "thinker".to_string(),
        messages: vec![LlmMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }],
        stream: None,
        temperature: None,
        max_tokens: None,
    };

    let mut stream = open_llm_chat_stream(&socket_path, &AppId::new("notes"), params)
        .await
        .unwrap();

    let first = stream.next_event().await.unwrap().unwrap();
    assert_eq!(first.content_delta, "hel");
    assert_eq!(first.finish_reason, None);

    let second = stream.next_event().await.unwrap().unwrap();
    assert_eq!(second.content_delta, "lo");
    assert_eq!(second.finish_reason.as_deref(), Some("stop"));
    assert_eq!(second.usage.unwrap().total_tokens, 5);

    let end = stream.next_event().await.unwrap();
    assert!(end.is_none());

    server_task.await.unwrap();
}

#[tokio::test]
async fn collect_llm_chat_aggregates_stream_chunks() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();
        let Frame::Request {
            request_id,
            session_id,
            ..
        } = frame
        else {
            panic!("expected request frame");
        };

        for chunk in [
            LlmChatStreamChunk {
                content_delta: "hello ".to_string(),
                finish_reason: None,
                usage: None,
            },
            LlmChatStreamChunk {
                content_delta: "world".to_string(),
                finish_reason: Some("stop".to_string()),
                usage: Some(LlmUsage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
            },
        ] {
            connection
                .send_frame(&Frame::Stream {
                    stream_id: StreamId(9),
                    request_id: Some(request_id),
                    session_id,
                    source: EndpointId::new("core"),
                    target: EndpointId::new("worker-sdk"),
                    payload: Payload {
                        bytes: postcard::to_stdvec(&chunk).unwrap(),
                        content_type: None,
                        schema: None,
                        metadata: FrameMetadata::new(),
                    },
                    end: false,
                    metadata: FrameMetadata::new(),
                })
                .await
                .unwrap();
        }

        connection
            .send_frame(&Frame::Stream {
                stream_id: StreamId(9),
                request_id: Some(request_id),
                session_id,
                source: EndpointId::new("core"),
                target: EndpointId::new("worker-sdk"),
                payload: Payload {
                    bytes: Vec::new(),
                    content_type: None,
                    schema: None,
                    metadata: FrameMetadata::new(),
                },
                end: true,
                metadata: FrameMetadata::new(),
            })
            .await
            .unwrap();
    });

    let params = LlmChatParams {
        role: "coder".to_string(),
        messages: vec![LlmMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }],
        stream: None,
        temperature: Some(0.1),
        max_tokens: Some(128),
    };

    let response: LlmChatResponse = collect_llm_chat(&socket_path, &AppId::new("notes"), params)
        .await
        .unwrap();

    assert_eq!(response.content, "hello world");
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    assert_eq!(response.usage.unwrap().total_tokens, 30);

    server_task.await.unwrap();
}

#[tokio::test]
async fn call_llm_embeddings_decodes_typed_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();
        let Frame::Request {
            request_id,
            payload,
            ..
        } = frame
        else {
            panic!("expected request frame");
        };

        let request = decode_capability_request(&payload).unwrap();
        assert_eq!(request.capability, "llm");
        assert_eq!(request.method, "embeddings");

        let params: LlmEmbeddingsParams = postcard::from_bytes(&request.params).unwrap();
        assert_eq!(params.role, "collector");
        assert_eq!(params.input, vec!["alpha", "beta"]);

        let result = LlmEmbeddingsResponse {
            embeddings: vec![vec![0.1, 0.2], vec![0.3, 0.4]],
        };

        let response = Frame::Response {
            request_id,
            session_id: SessionId(1),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker-sdk"),
            payload: encode_capability_response(&CapabilityResponse {
                result: Ok(postcard::to_stdvec(&result).unwrap()),
            })
            .unwrap(),
            metadata: FrameMetadata::new(),
        };
        connection.send_frame(&response).await.unwrap();
    });

    let response = call_llm_embeddings(
        &socket_path,
        &AppId::new("notes"),
        LlmEmbeddingsParams {
            role: "collector".to_string(),
            input: vec!["alpha".to_string(), "beta".to_string()],
        },
    )
    .await
    .unwrap();

    assert_eq!(response.embeddings, vec![vec![0.1, 0.2], vec![0.3, 0.4]]);
    server_task.await.unwrap();
}

#[tokio::test]
async fn call_llm_images_decodes_typed_response() {
    let (_root, socket_path) = socket_path();
    let listener = IpcListener::bind(&socket_path).await.unwrap();

    let server_task = tokio::spawn(async move {
        let mut connection = listener.accept().await.unwrap();
        let frame = connection.recv_frame().await.unwrap().unwrap();
        let Frame::Request {
            request_id,
            payload,
            ..
        } = frame
        else {
            panic!("expected request frame");
        };

        let request = decode_capability_request(&payload).unwrap();
        assert_eq!(request.capability, "llm");
        assert_eq!(request.method, "images");

        let params: LlmImagesParams = postcard::from_bytes(&request.params).unwrap();
        assert_eq!(params.role, "designer");
        assert_eq!(params.prompt, "draw a cat");
        assert_eq!(params.size.as_deref(), Some("1024x1024"));
        assert_eq!(params.n, Some(2));

        let result = LlmImagesResponse {
            urls: vec![
                "https://example.com/a.png".to_string(),
                "https://example.com/b.png".to_string(),
            ],
        };

        let response = Frame::Response {
            request_id,
            session_id: SessionId(1),
            source: EndpointId::new("core"),
            target: EndpointId::new("worker-sdk"),
            payload: encode_capability_response(&CapabilityResponse {
                result: Ok(postcard::to_stdvec(&result).unwrap()),
            })
            .unwrap(),
            metadata: FrameMetadata::new(),
        };
        connection.send_frame(&response).await.unwrap();
    });

    let response = call_llm_images(
        &socket_path,
        &AppId::new("notes"),
        LlmImagesParams {
            role: "designer".to_string(),
            prompt: "draw a cat".to_string(),
            size: Some("1024x1024".to_string()),
            n: Some(2),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response.urls,
        vec![
            "https://example.com/a.png".to_string(),
            "https://example.com/b.png".to_string(),
        ]
    );
    server_task.await.unwrap();
}
