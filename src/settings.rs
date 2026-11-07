use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_CROSSFADE_SECS: f32 = 5.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub library_root: Option<PathBuf>,
    /// Devices are remembered by name, never as "system default": macOS moves
    /// the default onto AirPods when they connect, and the speakers' audio
    /// must not follow it into the headphones.
    pub master_device: Option<String>,
    pub cue_device: Option<String>,
    pub crossfade_secs: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_root: None,
            master_device: None,
            cue_device: None,
            crossfade_secs: DEFAULT_CROSSFADE_SECS,
        }
    }
}

pub fn support_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/MusicJam")
}

pub fn load() -> Settings {
    load_from(&support_dir())
}

pub fn save(settings: &Settings) -> io::Result<()> {
    save_to(&support_dir(), settings)
}

fn load_from(dir: &Path) -> Settings {
    std::fs::read_to_string(dir.join("settings.toml"))
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_to(dir: &Path, settings: &Settings) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = toml::to_string_pretty(settings).map_err(io::Error::other)?;
    // Write then rename, so a crash mid-write never leaves half a file.
    let tmp = dir.join("settings.toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, dir.join("settings.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_from(dir.path()).crossfade_secs, 5.0);

        let settings = Settings {
            library_root: Some("/Music".into()),
            master_device: Some("MacBook Pro Speakers".into()),
            cue_device: Some("AirPods".into()),
            crossfade_secs: 7.5,
        };
        save_to(dir.path(), &settings).unwrap();
        assert_eq!(load_from(dir.path()), settings);
    }
}
