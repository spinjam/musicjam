use std::path::Path;

use lofty::prelude::*;

pub const AUDIO_EXTENSIONS: [&str; 3] = ["mp3", "flac", "wav"];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| e.eq_ignore_ascii_case(a)))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meta {
    pub artist: String,
    pub album: String,
    pub title: String,
    pub track_no: Option<u32>,
    pub duration_secs: f32,
}

/// What the folder layout says about a file: `Artist/Album/03 - Title.ext`,
/// or `Artist - Title.ext` when the file sits higher up.
pub fn meta_from_path(root: &Path, path: &Path) -> Meta {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let dirs: Vec<String> = rel
        .parent()
        .map(|p| p.iter().map(|c| c.to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    let mut artist = if dirs.len() >= 2 { dirs[dirs.len() - 2].clone() } else { String::new() };
    let album = dirs.last().cloned().unwrap_or_default();

    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let (track_no, rest) = split_track_number(&stem);
    let mut title = rest.trim().to_string();
    if artist.is_empty()
        && let Some((a, t)) = title.split_once(" - ")
    {
        artist = a.trim().to_string();
        title = t.trim().to_string();
    }
    if title.is_empty() {
        title = stem;
    }
    Meta { artist, album, title, track_no, duration_secs: 0.0 }
}

fn split_track_number(stem: &str) -> (Option<u32>, &str) {
    let digits = stem.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return (None, stem);
    }
    let rest = &stem[digits..];
    let stripped = rest.trim_start_matches([' ', '-', '.', '_']);
    // "1979.mp3" is a title, not track 1979; "03 - X" and "03. X" are tracks.
    if stripped.len() == rest.len() || stripped.is_empty() {
        return (None, stem);
    }
    (stem[..digits].parse().ok(), stripped)
}

/// Tags win field by field; whatever they lack comes from the path.
pub fn read_meta(root: &Path, path: &Path) -> Meta {
    let mut meta = meta_from_path(root, path);
    let Ok(file) = lofty::read_from_path(path) else {
        return meta;
    };
    meta.duration_secs = file.properties().duration().as_secs_f32();
    if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
        let set = |field: &mut String, value: Option<std::borrow::Cow<'_, str>>| {
            if let Some(v) = value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
                *field = v;
            }
        };
        set(&mut meta.title, tag.title());
        set(&mut meta.artist, tag.artist());
        set(&mut meta.album, tag.album());
        if let Some(n) = tag.track() {
            meta.track_no = Some(n);
        }
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_from_path() {
        let root = Path::new("/Music");
        let m = meta_from_path(root, Path::new("/Music/Blur/The Ballad of Darren/03 - The Narcissist.wav"));
        assert_eq!(m.artist, "Blur");
        assert_eq!(m.album, "The Ballad of Darren");
        assert_eq!(m.title, "The Narcissist");
        assert_eq!(m.track_no, Some(3));
    }

    #[test]
    fn fallback_artist_dash_title_and_numeric_titles() {
        let root = Path::new("/Music");
        let m = meta_from_path(root, Path::new("/Music/Blur - Song 2.mp3"));
        assert_eq!((m.artist.as_str(), m.title.as_str()), ("Blur", "Song 2"));
        let m = meta_from_path(root, Path::new("/Music/Smashing Pumpkins/Mellon Collie/1979.flac"));
        assert_eq!((m.title.as_str(), m.track_no), ("1979", None));
    }

    #[test]
    fn audio_extensions_case_insensitive() {
        assert!(is_audio(Path::new("a.FLAC")));
        assert!(!is_audio(Path::new("cover.jpg")));
    }
}
