use kunkka_core::theme::{ThemeConfig, ThemeFlavor, ThemeManager, ThemeSchedule};
use std::fs;
use tempfile::TempDir;

fn setup_theme_dir() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config").join("kunkka");
    fs::create_dir_all(&config_dir).unwrap();
    (tmp, config_dir)
}

#[test]
fn load_default_theme_when_no_config() {
    let (_tmp, config_dir) = setup_theme_dir();
    let manager = ThemeManager::load_from_dir(&config_dir).unwrap();
    assert_eq!(manager.active_flavor(), ThemeFlavor::Macchiato);
}

#[test]
fn load_existing_theme_config() {
    let (_tmp, config_dir) = setup_theme_dir();
    let config = ThemeConfig {
        active_flavor: ThemeFlavor::Latte,
        schedule: None,
        hooks: vec![],
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    fs::write(config_dir.join("theme.json"), json).unwrap();

    let manager = ThemeManager::load_from_dir(&config_dir).unwrap();
    assert_eq!(manager.active_flavor(), ThemeFlavor::Latte);
}

#[test]
fn switch_flavor_updates_config() {
    let (_tmp, config_dir) = setup_theme_dir();
    let mut manager = ThemeManager::load_from_dir(&config_dir).unwrap();

    manager.switch_flavor(ThemeFlavor::Latte).unwrap();
    assert_eq!(manager.active_flavor(), ThemeFlavor::Latte);

    // Verify config file was written
    let json = fs::read_to_string(config_dir.join("theme.json")).unwrap();
    let config: ThemeConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.active_flavor, ThemeFlavor::Latte);
}

#[test]
fn palette_returns_correct_colors() {
    let (_tmp, config_dir) = setup_theme_dir();
    let manager = ThemeManager::load_from_dir(&config_dir).unwrap();

    let palette = manager.palette();
    assert_eq!(palette.base, "#24273a"); // Macchiato default
    assert_eq!(palette.text, "#cad3f5");
}

#[test]
fn switch_flavor_updates_palette() {
    let (_tmp, config_dir) = setup_theme_dir();
    let mut manager = ThemeManager::load_from_dir(&config_dir).unwrap();

    manager.switch_flavor(ThemeFlavor::Latte).unwrap();
    let palette = manager.palette();
    assert_eq!(palette.base, "#eff1f5"); // Latte
    assert_eq!(palette.text, "#4c4f69");
}

#[test]
fn toggle_flavor_switches_between_latte_and_macchiato() {
    let (_tmp, config_dir) = setup_theme_dir();
    let mut manager = ThemeManager::load_from_dir(&config_dir).unwrap();

    assert_eq!(manager.active_flavor(), ThemeFlavor::Macchiato);
    manager.toggle_flavor().unwrap();
    assert_eq!(manager.active_flavor(), ThemeFlavor::Latte);
    manager.toggle_flavor().unwrap();
    assert_eq!(manager.active_flavor(), ThemeFlavor::Macchiato);
}

#[test]
fn schedule_check_returns_new_flavor_at_scheduled_time() {
    let (_tmp, config_dir) = setup_theme_dir();
    let mut manager = ThemeManager::load_from_dir(&config_dir).unwrap();

    // Set schedule
    manager
        .update_schedule(Some(ThemeSchedule {
            light_at: "07:00".to_string(),
            dark_at: "19:00".to_string(),
        }))
        .unwrap();

    // At 06:00, should be dark (Macchiato)
    let result = manager.check_schedule_at("06:00");
    assert!(result.is_none()); // Already Macchiato, no change

    // At 08:00, should be light (Latte)
    let result = manager.check_schedule_at("08:00");
    assert_eq!(result, Some(ThemeFlavor::Latte));
    // Simulate applying the schedule change
    manager.switch_flavor(ThemeFlavor::Latte).unwrap();

    // At 20:00, should be dark (Macchiato)
    let result = manager.check_schedule_at("20:00");
    assert_eq!(result, Some(ThemeFlavor::Macchiato));
}
