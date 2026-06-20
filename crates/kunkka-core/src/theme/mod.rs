use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeFlavor {
    Latte,
    Macchiato,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub active_flavor: ThemeFlavor,
    pub schedule: Option<ThemeSchedule>,
    pub hooks: Vec<ThemeHook>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSchedule {
    pub light_at: String,
    pub dark_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeHook {
    pub name: String,
    pub script: String,
}

#[derive(Debug, Clone)]
pub struct CatppuccinPalette {
    pub rosewater: &'static str,
    pub flamingo: &'static str,
    pub pink: &'static str,
    pub mauve: &'static str,
    pub red: &'static str,
    pub maroon: &'static str,
    pub peach: &'static str,
    pub yellow: &'static str,
    pub green: &'static str,
    pub teal: &'static str,
    pub sky: &'static str,
    pub sapphire: &'static str,
    pub blue: &'static str,
    pub lavender: &'static str,
    pub text: &'static str,
    pub subtext1: &'static str,
    pub subtext0: &'static str,
    pub overlay2: &'static str,
    pub overlay1: &'static str,
    pub overlay0: &'static str,
    pub surface2: &'static str,
    pub surface1: &'static str,
    pub surface0: &'static str,
    pub base: &'static str,
    pub mantle: &'static str,
    pub crust: &'static str,
}

impl ThemeFlavor {
    pub fn palette(&self) -> &'static CatppuccinPalette {
        match self {
            ThemeFlavor::Latte => &palette::LATTE,
            ThemeFlavor::Macchiato => &palette::MACCHIATO,
        }
    }
}

pub mod palette;

pub struct ThemeManager {
    config: ThemeConfig,
    config_path: PathBuf,
}

impl ThemeManager {
    pub fn load_from_dir(config_dir: &Path) -> Result<Self, std::io::Error> {
        let config_path = config_dir.join("theme.json");
        let config = if config_path.exists() {
            let json = std::fs::read_to_string(&config_path)?;
            serde_json::from_str(&json)?
        } else {
            ThemeConfig {
                active_flavor: ThemeFlavor::Macchiato,
                schedule: None,
                hooks: vec![],
            }
        };

        Ok(Self {
            config,
            config_path,
        })
    }

    pub fn active_flavor(&self) -> ThemeFlavor {
        self.config.active_flavor
    }

    pub fn palette(&self) -> &'static CatppuccinPalette {
        self.config.active_flavor.palette()
    }

    pub fn config(&self) -> &ThemeConfig {
        &self.config
    }

    pub fn switch_flavor(&mut self, flavor: ThemeFlavor) -> Result<(), std::io::Error> {
        self.config.active_flavor = flavor;
        self.save()
    }

    pub fn toggle_flavor(&mut self) -> Result<(), std::io::Error> {
        let new_flavor = match self.config.active_flavor {
            ThemeFlavor::Latte => ThemeFlavor::Macchiato,
            ThemeFlavor::Macchiato => ThemeFlavor::Latte,
        };
        self.switch_flavor(new_flavor)
    }

    pub fn update_schedule(
        &mut self,
        schedule: Option<ThemeSchedule>,
    ) -> Result<(), std::io::Error> {
        self.config.schedule = schedule;
        self.save()
    }

    pub fn check_schedule_at(&self, current_time: &str) -> Option<ThemeFlavor> {
        let schedule = self.config.schedule.as_ref()?;
        let (current_hour, current_min) = parse_hhmm(current_time)?;
        let (light_hour, light_min) = parse_hhmm(&schedule.light_at)?;
        let (dark_hour, dark_min) = parse_hhmm(&schedule.dark_at)?;

        let current_mins = current_hour * 60 + current_min;
        let light_mins = light_hour * 60 + light_min;
        let dark_mins = dark_hour * 60 + dark_min;

        let target = if light_mins < dark_mins {
            // Normal case: light during day, dark at night
            if current_mins >= light_mins && current_mins < dark_mins {
                ThemeFlavor::Latte
            } else {
                ThemeFlavor::Macchiato
            }
        } else {
            // Inverted case: dark during day, light at night
            if current_mins >= dark_mins && current_mins < light_mins {
                ThemeFlavor::Macchiato
            } else {
                ThemeFlavor::Latte
            }
        };

        if target == self.config.active_flavor {
            None
        } else {
            Some(target)
        }
    }

    fn save(&self) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(&self.config_path, json)
    }
}

fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let min: u32 = parts[1].parse().ok()?;
    if hour > 23 || min > 59 {
        return None;
    }
    Some((hour, min))
}
