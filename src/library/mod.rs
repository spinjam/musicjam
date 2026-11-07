pub mod scan;
pub mod search;
pub mod tree;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::Result;
use rusqlite::{Connection, params};

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: i64,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_no: Option<u32>,
    pub duration_secs: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanReport {
    Done {
        added: usize,
        updated: usize,
        removed: usize,
    },
    /// The folder is not reachable (an unplugged drive, say). The index is
    /// left as it was rather than emptied.
    RootMissing,
}

pub struct Library {
    conn: Connection,
    tracks: Vec<Track>,
}

impl Library {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(dir) = db_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                mtime INTEGER NOT NULL,
                size INTEGER NOT NULL,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                track_no INTEGER,
                duration REAL NOT NULL
            );",
        )?;
        let mut library = Self {
            conn,
            tracks: Vec::new(),
        };
        library.reload()?;
        Ok(library)
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn rescan(&mut self, root: &Path) -> Result<ScanReport> {
        if !root.is_dir() {
            return Ok(ScanReport::RootMissing);
        }
        let known: HashMap<String, (i64, i64)> = {
            let mut stmt = self.conn.prepare("SELECT path, mtime, size FROM tracks")?;
            stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?
                .collect::<rusqlite::Result<_>>()?
        };

        let (mut added, mut updated) = (0, 0);
        let mut seen = HashSet::new();
        let tx = self.conn.transaction()?;
        for entry in walkdir::WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if !entry.file_type().is_file() || !scan::is_audio(path) {
                continue;
            }
            let Ok(md) = entry.metadata() else { continue };
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            let size = md.len() as i64;
            let key = path.to_string_lossy().into_owned();
            seen.insert(key.clone());
            match known.get(&key) {
                Some(&(m, s)) if m == mtime && s == size => continue,
                Some(_) => updated += 1,
                None => added += 1,
            }
            let meta = scan::read_meta(root, path);
            tx.execute(
                "INSERT INTO tracks (path, mtime, size, title, artist, album, track_no, duration)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(path) DO UPDATE SET mtime=?2, size=?3, title=?4, artist=?5,
                    album=?6, track_no=?7, duration=?8",
                params![
                    key,
                    mtime,
                    size,
                    meta.title,
                    meta.artist,
                    meta.album,
                    meta.track_no,
                    meta.duration_secs
                ],
            )?;
        }
        let gone: Vec<&String> = known.keys().filter(|k| !seen.contains(*k)).collect();
        for path in &gone {
            tx.execute("DELETE FROM tracks WHERE path = ?1", [path])?;
        }
        tx.commit()?;
        self.reload()?;
        Ok(ScanReport::Done {
            added,
            updated,
            removed: gone.len(),
        })
    }

    fn reload(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, title, artist, album, track_no, duration FROM tracks ORDER BY path",
        )?;
        self.tracks = stmt
            .query_map([], |r| {
                Ok(Track {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                    title: r.get(2)?,
                    artist: r.get(3)?,
                    album: r.get(4)?,
                    track_no: r.get(5)?,
                    duration_secs: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"not really audio").unwrap();
    }

    #[test]
    fn rescan_detects_added_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        touch(&root, "Blur/The Ballad of Darren/03 - The Narcissist.wav");
        touch(&root, "Blur/Blur/02 - Song 2.mp3");
        touch(&root, "Blur/Blur/cover.jpg");
        let mut lib = Library::open(&dir.path().join("lib.sqlite")).unwrap();

        assert_eq!(
            lib.rescan(&root).unwrap(),
            ScanReport::Done {
                added: 2,
                updated: 0,
                removed: 0
            }
        );
        let t = lib
            .tracks()
            .iter()
            .find(|t| t.title == "The Narcissist")
            .unwrap();
        assert_eq!((t.artist.as_str(), t.track_no), ("Blur", Some(3)));

        std::fs::remove_file(root.join("Blur/Blur/02 - Song 2.mp3")).unwrap();
        assert_eq!(
            lib.rescan(&root).unwrap(),
            ScanReport::Done {
                added: 0,
                updated: 0,
                removed: 1
            }
        );
        assert_eq!(lib.tracks().len(), 1);
    }

    #[test]
    fn missing_root_keeps_index() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        touch(&root, "Blur/Blur/02 - Song 2.mp3");
        let db = dir.path().join("lib.sqlite");
        let mut lib = Library::open(&db).unwrap();
        lib.rescan(&root).unwrap();

        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(lib.rescan(&root).unwrap(), ScanReport::RootMissing);
        assert_eq!(Library::open(&db).unwrap().tracks().len(), 1);
    }
}
