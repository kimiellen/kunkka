use kunkka_core::theme::{ThemeConfig, ThemeFlavor, ThemeHook, ThemeSchedule};

#[test]
fn theme_flavor_serializes_to_lowercase() {
    assert_eq!(
        serde_json::to_string(&ThemeFlavor::Latte).unwrap(),
        "\"latte\""
    );
    assert_eq!(
        serde_json::to_string(&ThemeFlavor::Macchiato).unwrap(),
        "\"macchiato\""
    );
}

#[test]
fn theme_flavor_deserializes_from_lowercase() {
    assert_eq!(
        serde_json::from_str::<ThemeFlavor>("\"latte\"").unwrap(),
        ThemeFlavor::Latte
    );
    assert_eq!(
        serde_json::from_str::<ThemeFlavor>("\"macchiato\"").unwrap(),
        ThemeFlavor::Macchiato
    );
}

#[test]
fn theme_flavor_palette_returns_correct_colors() {
    let latte = ThemeFlavor::Latte.palette();
    assert_eq!(latte.base, "#eff1f5");
    assert_eq!(latte.text, "#4c4f69");

    let macchiato = ThemeFlavor::Macchiato.palette();
    assert_eq!(macchiato.base, "#24273a");
    assert_eq!(macchiato.text, "#cad3f5");
}

#[test]
fn theme_config_serializes_roundtrip() {
    let config = ThemeConfig {
        active_flavor: ThemeFlavor::Macchiato,
        schedule: Some(ThemeSchedule {
            light_at: "07:00".to_string(),
            dark_at: "19:00".to_string(),
        }),
        hooks: vec![ThemeHook {
            name: "kitty".to_string(),
            script: "~/.config/kunkka/hooks/kitty.sh".to_string(),
        }],
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: ThemeConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.active_flavor, ThemeFlavor::Macchiato);
    assert!(deserialized.schedule.is_some());
    assert_eq!(deserialized.hooks.len(), 1);
    assert_eq!(deserialized.hooks[0].name, "kitty");
}

#[test]
fn theme_config_parses_example_json() {
    let json = r#"{
        "active_flavor": "latte",
        "schedule": null,
        "hooks": []
    }"#;

    let config: ThemeConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.active_flavor, ThemeFlavor::Latte);
    assert!(config.schedule.is_none());
    assert!(config.hooks.is_empty());
}

#[test]
fn catppuccin_palette_has_26_colors() {
    let palette = ThemeFlavor::Latte.palette();
    // Verify all 26 colors are present by checking they're non-empty
    assert!(!palette.rosewater.is_empty());
    assert!(!palette.flamingo.is_empty());
    assert!(!palette.pink.is_empty());
    assert!(!palette.mauve.is_empty());
    assert!(!palette.red.is_empty());
    assert!(!palette.maroon.is_empty());
    assert!(!palette.peach.is_empty());
    assert!(!palette.yellow.is_empty());
    assert!(!palette.green.is_empty());
    assert!(!palette.teal.is_empty());
    assert!(!palette.sky.is_empty());
    assert!(!palette.sapphire.is_empty());
    assert!(!palette.blue.is_empty());
    assert!(!palette.lavender.is_empty());
    assert!(!palette.text.is_empty());
    assert!(!palette.subtext1.is_empty());
    assert!(!palette.subtext0.is_empty());
    assert!(!palette.overlay2.is_empty());
    assert!(!palette.overlay1.is_empty());
    assert!(!palette.overlay0.is_empty());
    assert!(!palette.surface2.is_empty());
    assert!(!palette.surface1.is_empty());
    assert!(!palette.surface0.is_empty());
    assert!(!palette.base.is_empty());
    assert!(!palette.mantle.is_empty());
    assert!(!palette.crust.is_empty());
}
