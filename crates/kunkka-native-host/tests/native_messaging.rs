use kunkka_native_host::native_messaging::{
    read_native_message, write_native_message, MAX_NATIVE_MESSAGE_LEN,
};
use std::io::Cursor;

#[test]
fn reads_length_prefixed_json_body() {
    let body = br#"{"id":"req-1","command":"ping"}"#;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(body);

    let mut reader = Cursor::new(bytes);
    let decoded = read_native_message(&mut reader).unwrap().unwrap();

    assert_eq!(decoded, body);
}

#[test]
fn writes_length_prefixed_json_body() {
    let mut output = Vec::new();
    let value = serde_json::json!({"id":"req-1","ok":true,"result":{"type":"pong"}});

    write_native_message(&mut output, &value).unwrap();

    let body_len = u32::from_le_bytes(output[0..4].try_into().unwrap()) as usize;
    let body = &output[4..];

    assert_eq!(body_len, body.len());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(body).unwrap(),
        value
    );
}

#[test]
fn rejects_oversized_native_message() {
    let oversized = (MAX_NATIVE_MESSAGE_LEN as u32) + 1;
    let mut reader = Cursor::new(oversized.to_le_bytes().to_vec());

    let err = read_native_message(&mut reader).unwrap_err();

    assert!(err.to_string().contains("native message too large"));
}
