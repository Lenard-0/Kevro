//! Config: parse, validate, defaults and file location.
//!
//! Parsing is all-or-nothing: if any part of the file is invalid, `parse`
//! returns an error and the caller keeps the previous config.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::SystemTime;
use std::{fs, io};

use global_hotkey::hotkey::HotKey;
use serde::Deserialize;

use crate::actions::Action;
use crate::focus::Direction;
use crate::snap::SnapAction;

pub const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub cycle_halves_to_thirds: bool,
    pub gap: f64,
    pub hide_if_frontmost: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { cycle_halves_to_thirds: false, gap: 0.0, hide_if_frontmost: false }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    apps: BTreeMap<String, String>,
    #[serde(default)]
    snap: BTreeMap<String, String>,
    #[serde(default)]
    focus: BTreeMap<String, String>,
    #[serde(default)]
    options: Options,
}

#[derive(Debug, Clone)]
pub struct Binding {
    /// The key string as written in the config, for log messages.
    pub key: String,
    pub hotkey: HotKey,
    pub action: Action,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub bindings: Vec<Binding>,
    pub options: Options,
}

pub fn parse(text: &str) -> Result<Config, String> {
    let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;

    if !raw.options.gap.is_finite() || raw.options.gap < 0.0 || raw.options.gap > 500.0 {
        return Err(format!("options.gap must be between 0 and 500, got {}", raw.options.gap));
    }

    let mut bindings = Vec::new();
    let mut seen: HashMap<u32, String> = HashMap::new();
    let mut errors = Vec::new();

    let mut add = |section: &str, key: &str, action: Result<Action, String>| {
        let hotkey = match HotKey::from_str(key) {
            Ok(h) => h,
            Err(e) => {
                errors.push(format!("[{section}] \"{key}\": invalid hotkey ({e})"));
                return;
            }
        };
        let action = match action {
            Ok(a) => a,
            Err(e) => {
                errors.push(format!("[{section}] \"{key}\": {e}"));
                return;
            }
        };
        if let Some(prev) = seen.insert(hotkey.id(), key.to_string()) {
            errors.push(format!("\"{key}\" is bound more than once (also \"{prev}\")"));
            return;
        }
        bindings.push(Binding { key: key.to_string(), hotkey, action });
    };

    for (key, target) in &raw.apps {
        let action = if target.trim().is_empty() {
            Err("app target is empty".to_string())
        } else {
            Ok(Action::App(target.trim().to_string()))
        };
        add("apps", key, action);
    }
    for (key, name) in &raw.snap {
        let action = SnapAction::from_name(name)
            .map(Action::Snap)
            .ok_or_else(|| format!("unknown snap action \"{name}\""));
        add("snap", key, action);
    }
    for (key, name) in &raw.focus {
        let action = Direction::from_name(name)
            .map(Action::Focus)
            .ok_or_else(|| format!("unknown focus action \"{name}\""));
        add("focus", key, action);
    }

    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(Config { bindings, options: raw.options })
}

pub fn config_dir() -> PathBuf {
    home().join(".config").join("kevro")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn log_dir() -> PathBuf {
    home().join("Library").join("Logs").join("kevro")
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Write the commented default config if no config file exists yet.
pub fn ensure_config_file() -> io::Result<bool> {
    let path = config_path();
    if path.exists() {
        return Ok(false);
    }
    fs::create_dir_all(config_dir())?;
    fs::write(&path, DEFAULT_CONFIG)?;
    Ok(true)
}

pub fn modified_time() -> Option<SystemTime> {
    fs::metadata(config_path()).and_then(|m| m.modified()).ok()
}

/// Read and parse the config file.
pub fn load() -> Result<Config, String> {
    let text = fs::read_to_string(config_path())
        .map_err(|e| format!("cannot read {}: {e}", config_path().display()))?;
    parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let cfg = parse(DEFAULT_CONFIG).expect("default config must parse");
        assert!(cfg.bindings.len() >= 15);
        assert!(cfg.bindings.iter().any(|b| b.action == Action::Snap(SnapAction::Restore)));
    }

    #[test]
    fn empty_config_is_valid() {
        let cfg = parse("").unwrap();
        assert!(cfg.bindings.is_empty());
        assert_eq!(cfg.options, Options::default());
    }

    #[test]
    fn rejects_unknown_action() {
        let err = parse("[snap]\n\"ctrl+alt+x\" = \"left_haf\"").unwrap_err();
        assert!(err.contains("left_haf"), "{err}");
    }

    #[test]
    fn rejects_bad_hotkey() {
        assert!(parse("[apps]\n\"ctrl+nope+x\" = \"com.apple.Safari\"").is_err());
    }

    #[test]
    fn rejects_duplicate_across_sections() {
        let text = "[apps]\n\"ctrl+alt+x\" = \"Safari\"\n[snap]\n\"alt+ctrl+x\" = \"maximize\"";
        assert!(parse(text).unwrap_err().contains("more than once"));
    }

    #[test]
    fn rejects_bad_toml_and_unknown_fields() {
        assert!(parse("[apps\n").is_err());
        assert!(parse("[options]\ngapp = 3").is_err());
        assert!(parse("[options]\ngap = -1").is_err());
    }
}
