use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::playlist::Playlist;

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
    /// Slider levels, 0..=1.
    pub master_volume: f32,
    pub cue_volume: f32,
    /// A theme by name; None follows the system's light or dark.
    pub theme: Option<String>,
    /// Every track brought to the same loudness.
    pub normalize: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            library_root: None,
            master_device: None,
            cue_device: None,
            crossfade_secs: DEFAULT_CROSSFADE_SECS,
            master_volume: 1.0,
            cue_volume: 0.8,
            theme: None,
            normalize: true,
        }
    }
}

pub fn support_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
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
    write_toml(dir, "settings.toml", settings)
}

/// Write then rename, so a crash mid-write never leaves half a file.
fn write_toml(dir: &Path, name: &str, value: &impl Serialize) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = toml::to_string_pretty(value).map_err(io::Error::other)?;
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, dir.join(name))
}

/// Tonight's setlist as it was left, saved or not.
pub fn load_session() -> Option<Playlist> {
    load_session_from(&support_dir())
}

pub fn save_session(playlist: &Playlist) -> io::Result<()> {
    write_toml(&support_dir(), "session.toml", playlist)
}

fn load_session_from(dir: &Path) -> Option<Playlist> {
    let text = std::fs::read_to_string(dir.join("session.toml")).ok()?;
    toml::from_str(&text).ok()
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
            master_volume: 0.6,
            cue_volume: 0.4,
            theme: Some("Neon".into()),
            normalize: false,
        };
        save_to(dir.path(), &settings).unwrap();
        assert_eq!(load_from(dir.path()), settings);

        // A file written before a setting existed still loads, with the default.
        std::fs::write(dir.path().join("settings.toml"), "crossfade_secs = 3.0\n").unwrap();
        let old = load_from(dir.path());
        assert_eq!((old.crossfade_secs, old.master_volume), (3.0, 1.0));
    }

    #[test]
    fn session_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_session_from(dir.path()).is_none());
        let mut p = Playlist::new();
        p.push("/m/a.flac".into());
        write_toml(dir.path(), "session.toml", &p).unwrap();
        assert_eq!(load_session_from(dir.path()), Some(p));
    }
}
