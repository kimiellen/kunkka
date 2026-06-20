use kunkka_protocol::core_control::{
    decode_control_message, encode_control_message, CoreControlMessage, CoreGetThemeRequest,
    CoreGetThemeResponse, CoreSetThemeRequest, CoreSetThemeResponse, ThemeChangedEvent,
    ThemeFlavor,
};

#[test]
fn get_theme_request_roundtrip() {
    let msg = CoreControlMessage::GetTheme(CoreGetThemeRequest);
    let payload = encode_control_message(&msg).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn get_theme_response_roundtrip() {
    let msg = CoreControlMessage::GetThemeResult(CoreGetThemeResponse {
        flavor: ThemeFlavor::Macchiato,
    });
    let payload = encode_control_message(&msg).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn set_theme_request_roundtrip() {
    let msg = CoreControlMessage::SetTheme(CoreSetThemeRequest {
        flavor: ThemeFlavor::Latte,
    });
    let payload = encode_control_message(&msg).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn set_theme_response_roundtrip() {
    let msg = CoreControlMessage::SetThemeResult(CoreSetThemeResponse);
    let payload = encode_control_message(&msg).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn theme_changed_event_roundtrip() {
    let msg = CoreControlMessage::ThemeChanged(ThemeChangedEvent {
        flavor: ThemeFlavor::Latte,
    });
    let payload = encode_control_message(&msg).unwrap();
    let decoded = decode_control_message(&payload).unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn theme_flavor_serializes_to_lowercase() {
    let msg = CoreControlMessage::GetThemeResult(CoreGetThemeResponse {
        flavor: ThemeFlavor::Latte,
    });
    let payload = encode_control_message(&msg).unwrap();
    let decoded = decode_control_message(&payload).unwrap();

    if let CoreControlMessage::GetThemeResult(resp) = decoded {
        assert_eq!(resp.flavor, ThemeFlavor::Latte);
    } else {
        panic!("expected GetThemeResult");
    }
}
