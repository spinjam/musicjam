//! What MusicJam keeps beside the track index, in the same database: track
//! analyses and saved setlists. A second connection, so it stays usable
//! while a rescan holds the library's.

use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, params};

use crate::audio::analysis::Analysis;

pub type Items = Vec<(PathBuf, Option<f32>, Option<f32>)>;

pub struct Store {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SavedList {
    pub id: i64,
    pub name: String,
}

/// What identifies a file's content well enough to reuse its analysis.
fn stamp(path: &Path) -> Option<(i64, i64)> {
    let md = std::fs::metadata(path).ok()?;
    let mtime = md
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some((mtime, md.len() as i64))
}

impl Store {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(dir) = db_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(db_path)?;
        // A rescan may be writing through the other connection.
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS analysis (
                path TEXT PRIMARY KEY,
                mtime INTEGER NOT NULL,
                size INTEGER NOT NULL,
                peaks BLOB NOT NULL,
                sound_start REAL NOT NULL,
                sound_end REAL NOT NULL,
                loudness REAL,
                peak REAL NOT NULL,
                duration REAL NOT NULL
            );
            CREATE TABLE IF NOT EXISTS playlists (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                updated INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS playlist_entries (
                playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                pos INTEGER NOT NULL,
                path TEXT NOT NULL,
                start REAL,
                end_at REAL,
                PRIMARY KEY (playlist_id, pos)
            );
            PRAGMA foreign_keys = ON;",
        )?;
        Ok(Self { conn })
    }

    /// The stored analysis, if the file has not changed since.
    pub fn analysis(&self, path: &Path) -> Option<Analysis> {
        let (mtime, size) = stamp(path)?;
        self.conn
            .query_row(
                "SELECT peaks, sound_start, sound_end, loudness, peak, duration FROM analysis
                 WHERE path = ?1 AND mtime = ?2 AND size = ?3",
                params![path.to_string_lossy(), mtime, size],
                |r| {
                    let blob: Vec<u8> = r.get(0)?;
                    Ok(Analysis {
                        peaks: blob
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .map(|b| f32::from_le_bytes(*b))
                            .collect(),
                        sound_start: r.get(1)?,
                        sound_end: r.get(2)?,
                        loudness: r.get(3)?,
                        peak: r.get(4)?,
                        duration: r.get(5)?,
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn put_analysis(&self, path: &Path, a: &Analysis) -> Result<()> {
        let Some((mtime, size)) = stamp(path) else {
            return Ok(());
        };
        let blob: Vec<u8> = a.peaks.iter().flat_map(|p| p.to_le_bytes()).collect();
        self.conn.execute(
            "INSERT OR REPLACE INTO analysis
             (path, mtime, size, peaks, sound_start, sound_end, loudness, peak, duration)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                path.to_string_lossy(),
                mtime,
                size,
                blob,
                a.sound_start,
                a.sound_end,
                a.loudness,
                a.peak,
                a.duration
            ],
        )?;
        Ok(())
    }

    pub fn playlists(&self) -> Result<Vec<SavedList>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name FROM playlists ORDER BY name COLLATE NOCASE")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SavedList {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn load_playlist(&self, id: i64) -> Result<(String, Items)> {
        let name: String =
            self.conn
                .query_row("SELECT name FROM playlists WHERE id = ?1", [id], |r| {
                    r.get(0)
                })?;
        let mut stmt = self.conn.prepare(
            "SELECT path, start, end_at FROM playlist_entries WHERE playlist_id = ?1 ORDER BY pos",
        )?;
        let items = stmt
            .query_map([id], |r| {
                Ok((PathBuf::from(r.get::<_, String>(0)?), r.get(1)?, r.get(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok((name, items))
    }

    /// Writes a setlist: over `id` when given, else as a new one. Returns
    /// its id. A name already used by another setlist is refused.
    pub fn save_playlist(&mut self, id: Option<i64>, name: &str, items: &Items) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            bail!("la scaletta ha bisogno di un nome");
        }
        let taken: Option<i64> = self
            .conn
            .query_row("SELECT id FROM playlists WHERE name = ?1", [name], |r| {
                r.get(0)
            })
            .optional()?;
        if taken.is_some_and(|t| Some(t) != id) {
            bail!("esiste già una scaletta «{name}»");
        }
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let tx = self.conn.transaction()?;
        let id = match id {
            Some(id) => {
                tx.execute(
                    "UPDATE playlists SET name = ?2, updated = ?3 WHERE id = ?1",
                    params![id, name, now],
                )?;
                tx.execute("DELETE FROM playlist_entries WHERE playlist_id = ?1", [id])?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO playlists (name, updated) VALUES (?1, ?2)",
                    params![name, now],
                )?;
                tx.last_insert_rowid()
            }
        };
        for (pos, (path, start, end)) in items.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_entries (playlist_id, pos, path, start, end_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, pos as i64, path.to_string_lossy(), start, end],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            bail!("la scaletta ha bisogno di un nome");
        }
        match self.conn.execute(
            "UPDATE playlists SET name = ?2 WHERE id = ?1",
            params![id, name],
        ) {
            Err(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                bail!("esiste già una scaletta «{name}»")
            }
            r => r.map(|_| ()).map_err(Into::into),
        }
    }

    pub fn delete_playlist(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM playlist_entries WHERE playlist_id = ?1", [id])?;
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Items {
        vec![
            ("/m/a.flac".into(), None, Some(100.0)),
            ("/m/b.flac".into(), Some(3.0), None),
            ("/m/a.flac".into(), None, None),
        ]
    }

    #[test]
    fn playlists_save_load_rename_delete() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(&dir.path().join("lib.sqlite")).unwrap();
        let id = s.save_playlist(None, " Sabato ", &items()).unwrap();
        assert_eq!(s.load_playlist(id).unwrap(), ("Sabato".into(), items()));

        let mut shorter = items();
        shorter.pop();
        assert_eq!(s.save_playlist(Some(id), "Sabato", &shorter).unwrap(), id);
        assert_eq!(s.load_playlist(id).unwrap().1, shorter);

        assert!(
            s.save_playlist(None, "Sabato", &items()).is_err(),
            "name taken"
        );
        assert!(s.save_playlist(None, "  ", &items()).is_err());
        let other = s.save_playlist(None, "Domenica", &items()).unwrap();
        assert!(s.rename_playlist(other, "Sabato").is_err());
        s.rename_playlist(other, "Anteprima").unwrap();
        let names: Vec<String> = s.playlists().unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Anteprima", "Sabato"]);

        s.delete_playlist(id).unwrap();
        assert_eq!(s.playlists().unwrap().len(), 1);
        assert!(s.load_playlist(id).is_err());
    }

    #[test]
    fn analysis_is_reused_until_the_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("lib.sqlite")).unwrap();
        let file = dir.path().join("a.wav");
        std::fs::write(&file, b"1234").unwrap();
        let a = Analysis {
            peaks: vec![0.1, 0.5, 1.0],
            sound_start: 0.5,
            sound_end: 9.0,
            loudness: Some(-11.0),
            peak: 0.98,
            duration: 10.0,
        };
        assert!(s.analysis(&file).is_none());
        s.put_analysis(&file, &a).unwrap();
        assert_eq!(s.analysis(&file), Some(a));
        std::fs::write(&file, b"123456").unwrap();
        assert!(s.analysis(&file).is_none(), "size changed");
    }
}
