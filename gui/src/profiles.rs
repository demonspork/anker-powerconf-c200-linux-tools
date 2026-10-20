//! Named settings profiles stored as plain "control = value" text files.
//!
//! The value syntax is the CLI's, so a profile saved here can be re-applied with
//! `anker-powerconf-c200-linux-tools apply-profile <file>` (for example from a udev rule).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::controls::ControlValue;
use crate::ffi;

pub const EXTENSION: &str = "profile";

pub struct Profile {
    pub model: Option<String>,
    pub entries: Vec<(String, String)>,
}

pub struct ProfileStore {
    dir: PathBuf,
}

impl ProfileStore {
    /// `$XDG_CONFIG_HOME/anker-webcam-gui/profiles`, falling back to `~/.config`.
    pub fn new() -> ProfileStore {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        ProfileStore {
            dir: base.join("anker-webcam-gui").join("profiles"),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.{EXTENSION}"))
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some(EXTENSION) {
                    return None;
                }
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
            })
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    pub fn save(&self, name: &str, model: &str, entries: &[(String, String)]) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let mut text = String::from("# Anker webcam settings profile\n");
        text.push_str(&format!("model = {model}\n"));
        for (control, value) in entries {
            text.push_str(&format!("{control} = {value}\n"));
        }
        fs::write(self.path(name), text)
    }

    pub fn load(&self, name: &str) -> io::Result<Profile> {
        parse_profile(&fs::read_to_string(self.path(name))?)
    }

    pub fn delete(&self, name: &str) -> io::Result<()> {
        fs::remove_file(self.path(name))
    }
}

pub fn parse_profile(text: &str) -> io::Result<Profile> {
    let mut profile = Profile {
        model: None,
        entries: Vec::new(),
    };
    for (number, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: expected 'control = value'", number + 1),
            ));
        };
        let (key, value) = (key.trim(), value.trim());
        if key == "model" {
            profile.model = Some(value.to_string());
        } else {
            profile.entries.push((key.to_string(), value.to_string()));
        }
    }
    Ok(profile)
}

/// Profile names become file names: letters, digits, space, dash, underscore and dot only.
pub fn sanitize_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned.starts_with('.') {
        None
    } else {
        Some(cleaned)
    }
}

fn on_off(value: bool) -> &'static str {
    if value {
        "on"
    } else {
        "off"
    }
}

/// Serialise a value in the syntax the CLI's `set` command accepts.
pub fn value_to_text(value: &ControlValue) -> String {
    match value {
        ControlValue::VendorBool(v) | ControlValue::V4l2Bool(v) => on_off(*v).to_string(),
        ControlValue::VendorU8(v) => v.to_string(),
        ControlValue::Fov(v) => v.to_string(),
        ControlValue::AreaMetering(a) => format!(
            "{},{},{},{},{}",
            on_off(a.enable),
            a.x,
            a.y,
            a.width,
            a.height
        ),
        ControlValue::EnableU8(e) => format!("{},{}", on_off(e.enable), e.value),
        ControlValue::V4l2Int { value, .. } | ControlValue::V4l2Menu { value, .. } => {
            value.to_string()
        }
    }
}

fn parse_bool(text: &str) -> Option<bool> {
    match text.to_ascii_lowercase().as_str() {
        "1" | "on" | "true" | "yes" => Some(true),
        "0" | "off" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn split_list(text: &str) -> Vec<&str> {
    text.split(|c: char| c == ',' || c.is_whitespace() || c == ':' || c == '=' || c == 'x')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse profile text for a control, using `current` for the kind and to keep
/// fields the text does not mention (the CLI's pre-fill behaviour).
pub fn text_to_value(text: &str, current: &ControlValue) -> Option<ControlValue> {
    let text = text.trim();
    match current {
        ControlValue::VendorBool(_) => Some(ControlValue::VendorBool(parse_bool(text)?)),
        ControlValue::V4l2Bool(_) => Some(ControlValue::V4l2Bool(parse_bool(text)?)),
        ControlValue::VendorU8(_) => Some(ControlValue::VendorU8(text.parse().ok()?)),
        ControlValue::Fov(_) => {
            let value = match text.to_ascii_lowercase().as_str() {
                "narrow" => 65,
                "medium" => 78,
                "wide" => 95,
                other => other.parse().ok()?,
            };
            Some(ControlValue::Fov(value))
        }
        ControlValue::AreaMetering(current) => {
            let parts = split_list(text);
            let mut area = *current;
            area.enable = parse_bool(parts.first()?)?;
            match parts.len() {
                1 => {}
                5 => {
                    area.x = parts[1].parse().ok()?;
                    area.y = parts[2].parse().ok()?;
                    area.width = parts[3].parse().ok()?;
                    area.height = parts[4].parse().ok()?;
                }
                _ => return None,
            }
            if area.enable && (area.width == 0 || area.height == 0) {
                area = ffi::AreaMetering {
                    enable: true,
                    x: 1600,
                    y: 900,
                    width: 640,
                    height: 360,
                };
            }
            Some(ControlValue::AreaMetering(area))
        }
        ControlValue::EnableU8(current) => {
            let parts = split_list(text);
            let mut value = *current;
            match parts.as_slice() {
                [single] => {
                    if let Some(enable) = parse_bool(single) {
                        value.enable = enable;
                    } else {
                        value.value = single.parse().ok()?;
                    }
                }
                [enable, number] => {
                    value.enable = parse_bool(enable)?;
                    value.value = number.parse().ok()?;
                }
                _ => return None,
            }
            Some(ControlValue::EnableU8(value))
        }
        ControlValue::V4l2Int {
            min,
            max,
            step,
            default,
            ..
        } => Some(ControlValue::V4l2Int {
            value: text.parse().ok()?,
            min: *min,
            max: *max,
            step: *step,
            default: *default,
        }),
        ControlValue::V4l2Menu { items, .. } => Some(ControlValue::V4l2Menu {
            value: text.parse().ok()?,
            items: items.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_values() {
        let area = ControlValue::AreaMetering(ffi::AreaMetering {
            enable: true,
            x: 1600,
            y: 900,
            width: 640,
            height: 360,
        });
        assert_eq!(value_to_text(&area), "on,1600,900,640,360");
        let parsed = text_to_value(
            "on,1600,900,640,360",
            &ControlValue::AreaMetering(Default::default()),
        )
        .unwrap();
        assert_eq!(value_to_text(&parsed), "on,1600,900,640,360");

        let e = ControlValue::EnableU8(ffi::EnableU8 {
            enable: true,
            value: 35,
        });
        assert_eq!(value_to_text(&e), "on,35");
        let kept = text_to_value("off", &e).unwrap();
        assert_eq!(value_to_text(&kept), "off,35");
        let bare = text_to_value("20", &e).unwrap();
        assert_eq!(value_to_text(&bare), "on,20");

        assert_eq!(
            value_to_text(&text_to_value("wide", &ControlValue::Fov(0)).unwrap()),
            "95"
        );
        assert!(text_to_value("banana", &ControlValue::VendorBool(false)).is_none());
    }

    #[test]
    fn parse_profile_text() {
        let profile =
            parse_profile("# comment\nmodel = AnkerWork C310\nfov = 95\n\nface_exposure = on,35\n")
                .unwrap();
        assert_eq!(profile.model.as_deref(), Some("AnkerWork C310"));
        assert_eq!(
            profile.entries,
            vec![
                ("fov".into(), "95".into()),
                ("face_exposure".into(), "on,35".into())
            ]
        );
        assert!(parse_profile("garbage line").is_err());
    }

    #[test]
    fn names() {
        assert_eq!(
            sanitize_name("  Meeting room / day  ").as_deref(),
            Some("Meeting room  day")
        );
        assert!(sanitize_name("../etc").is_none());
        assert!(sanitize_name("   ").is_none());
    }
}
