//! The open setlist: the working copy that plays as the queue. Changes stay
//! here, marked as unsaved, until the user saves them.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Stable within a working copy: the marks follow it through moves.
    pub uid: u64,
    /// By path, not by index id: it survives the index being rebuilt.
    pub path: PathBuf,
    /// Chosen by hand; None takes the analysis' sound start and end.
    pub start: Option<f32>,
    pub end: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Playlist {
    /// The saved setlist this is a copy of; None for a new one.
    pub id: Option<i64>,
    pub name: Option<String>,
    pub entries: Vec<Entry>,
    /// Changed since it was opened or saved.
    pub dirty: bool,
    /// The entry on deck A and the one loaded (or loading) in deck B.
    pub on_air: Option<u64>,
    pub next: Option<u64>,
    pub played: HashSet<u64>,
    next_uid: u64,
}

impl Playlist {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn opened(id: i64, name: String, items: Vec<(PathBuf, Option<f32>, Option<f32>)>) -> Self {
        let mut p = Self {
            id: Some(id),
            name: Some(name),
            ..Self::default()
        };
        for (path, start, end) in items {
            let uid = p.uid();
            p.entries.push(Entry {
                uid,
                path,
                start,
                end,
            });
        }
        p
    }

    fn uid(&mut self) -> u64 {
        self.next_uid += 1;
        self.next_uid
    }

    pub fn index_of(&self, uid: u64) -> Option<usize> {
        self.entries.iter().position(|e| e.uid == uid)
    }

    pub fn get(&self, uid: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.uid == uid)
    }

    /// Adds a track at the end.
    pub fn push(&mut self, path: PathBuf) -> u64 {
        let uid = self.uid();
        self.entries.push(Entry {
            uid,
            path,
            start: None,
            end: None,
        });
        self.dirty = true;
        uid
    }

    /// Puts a track where the next one is (that one moves down a place), or
    /// right after the one on air, and makes it the next.
    pub fn insert_as_next(&mut self, path: PathBuf) -> u64 {
        let at = self
            .next
            .and_then(|n| self.index_of(n))
            .or_else(|| self.on_air.and_then(|a| self.index_of(a)).map(|i| i + 1))
            .unwrap_or_else(|| self.first_unplayed_index());
        let uid = self.uid();
        self.entries.insert(
            at,
            Entry {
                uid,
                path,
                start: None,
                end: None,
            },
        );
        self.dirty = true;
        self.next = Some(uid);
        uid
    }

    fn first_unplayed_index(&self) -> usize {
        self.entries
            .iter()
            .rposition(|e| self.played.contains(&e.uid) || Some(e.uid) == self.on_air)
            .map_or(0, |i| i + 1)
    }

    /// Removes an entry; true when it was the one in deck B.
    pub fn remove(&mut self, uid: u64) -> bool {
        let Some(i) = self.index_of(uid) else {
            return false;
        };
        self.entries.remove(i);
        self.dirty = true;
        self.played.remove(&uid);
        if self.on_air == Some(uid) {
            self.on_air = None;
        }
        let was_next = self.next == Some(uid);
        if was_next {
            self.next = None;
        }
        was_next
    }

    /// Moves an entry one place up (-1) or down (+1).
    pub fn shift(&mut self, uid: u64, by: isize) {
        let Some(i) = self.index_of(uid) else { return };
        let j = i as isize + by;
        if j < 0 || j as usize >= self.entries.len() {
            return;
        }
        self.entries.swap(i, j as usize);
        self.dirty = true;
    }

    pub fn set_start(&mut self, uid: u64, secs: Option<f32>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.uid == uid)
            && e.start != secs
        {
            e.start = secs;
            self.dirty = true;
        }
    }

    pub fn set_end(&mut self, uid: u64, secs: Option<f32>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.uid == uid)
            && e.end != secs
        {
            e.end = secs;
            self.dirty = true;
        }
    }

    /// The first entry after `after` (or from the top) whose file is there.
    pub fn following(&self, after: Option<u64>, available: impl Fn(&Path) -> bool) -> Option<u64> {
        let from = match after {
            Some(uid) => self.index_of(uid)? + 1,
            None => 0,
        };
        self.entries[from.min(self.entries.len())..]
            .iter()
            .find(|e| available(&e.path))
            .map(|e| e.uid)
    }

    /// Where to carry on from: the first entry after anything on air or
    /// already played.
    pub fn resume_point(&self, available: impl Fn(&Path) -> bool) -> Option<u64> {
        let from = self.first_unplayed_index();
        self.entries[from..]
            .iter()
            .find(|e| available(&e.path))
            .map(|e| e.uid)
    }

    /// Deck B went on air: it becomes A, and the entry after it is the new
    /// next (returned, for the caller to load).
    pub fn advance(&mut self, available: impl Fn(&Path) -> bool) -> Option<u64> {
        if let Some(a) = self.on_air.take() {
            self.played.insert(a);
        }
        self.on_air = self.next.take();
        self.next = self.following(self.on_air, available);
        self.next
    }

    /// Something from outside the setlist went straight onto deck A.
    pub fn leave_air(&mut self) {
        if let Some(a) = self.on_air.take() {
            self.played.insert(a);
        }
    }

    /// The saved version was written: this copy now matches it.
    pub fn mark_saved(&mut self, id: i64, name: String) {
        self.id = Some(id);
        self.name = Some(name);
        self.dirty = false;
    }

    /// Back to the saved entries. The marks stay on the first entry with
    /// the same file, when there still is one.
    pub fn revert_to(&mut self, items: Vec<(PathBuf, Option<f32>, Option<f32>)>) {
        let path_of = |uid: Option<u64>| uid.and_then(|u| self.get(u)).map(|e| e.path.clone());
        let (air, next) = (path_of(self.on_air), path_of(self.next));
        let played: HashSet<PathBuf> = self
            .played
            .iter()
            .filter_map(|&u| self.get(u).map(|e| e.path.clone()))
            .collect();
        let mut fresh = Playlist::opened(self.id.unwrap_or_default(), String::new(), items);
        fresh.id = self.id;
        fresh.name = self.name.clone();
        let find = |p: &Option<PathBuf>| {
            p.as_ref()
                .and_then(|p| fresh.entries.iter().find(|e| &e.path == p))
                .map(|e| e.uid)
        };
        fresh.on_air = find(&air);
        fresh.next = find(&next).filter(|n| Some(*n) != fresh.on_air);
        fresh.played = fresh
            .entries
            .iter()
            .filter(|e| played.contains(&e.path))
            .map(|e| e.uid)
            .collect();
        *self = fresh;
    }

    /// Path, start and end of every entry, in order: what gets saved.
    pub fn items(&self) -> Vec<(PathBuf, Option<f32>, Option<f32>)> {
        self.entries
            .iter()
            .map(|e| (e.path.clone(), e.start, e.end))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: &Path) -> bool {
        true
    }

    fn list(n: usize) -> Playlist {
        Playlist::opened(
            1,
            "Sabato".into(),
            (0..n)
                .map(|i| (PathBuf::from(format!("/m/{i}.flac")), None, None))
                .collect(),
        )
    }

    fn paths(p: &Playlist) -> Vec<String> {
        p.entries
            .iter()
            .map(|e| e.path.file_stem().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    fn uid_at(p: &Playlist, i: usize) -> u64 {
        p.entries[i].uid
    }

    #[test]
    fn opened_is_clean_and_edits_mark_it() {
        let mut p = list(3);
        assert!(!p.dirty);
        p.shift(uid_at(&p, 0), 1);
        assert!(p.dirty);
        assert_eq!(paths(&p), ["1", "0", "2"]);
    }

    #[test]
    fn marks_do_not_dirty() {
        let mut p = list(3);
        p.next = p.resume_point(all);
        p.advance(all);
        p.advance(all);
        assert!(!p.dirty);
    }

    #[test]
    fn advance_walks_the_list_and_skips_missing_files() {
        let mut p = list(4);
        p.next = p.resume_point(all);
        assert_eq!(p.next, Some(uid_at(&p, 0)));
        let missing = |path: &Path| !path.ends_with("2.flac");
        assert_eq!(p.advance(missing), Some(uid_at(&p, 1)));
        assert_eq!(p.on_air, Some(uid_at(&p, 0)));
        assert_eq!(p.advance(missing), Some(uid_at(&p, 3)));
        assert!(p.played.contains(&uid_at(&p, 0)));
        assert_eq!(p.advance(missing), None);
        assert_eq!(p.on_air, Some(uid_at(&p, 3)));
    }

    #[test]
    fn insert_as_next_pushes_the_old_next_down() {
        let mut p = list(3);
        p.next = p.resume_point(all);
        p.advance(all); // 0 on air, 1 next
        let new = p.insert_as_next("/m/x.flac".into());
        assert_eq!(paths(&p), ["0", "x", "1", "2"]);
        assert_eq!(p.next, Some(new));
        assert!(p.dirty);
        // And after it the list carries on with the old next.
        assert_eq!(p.advance(all), Some(uid_at(&p, 2)));
    }

    #[test]
    fn insert_as_next_with_nothing_next_goes_after_on_air() {
        let mut p = list(2);
        p.next = p.resume_point(all);
        p.advance(all);
        p.advance(all); // 1 on air, nothing next
        p.insert_as_next("/m/x.flac".into());
        assert_eq!(paths(&p), ["0", "1", "x"]);
        let mut empty = Playlist::new();
        empty.insert_as_next("/m/y.flac".into());
        assert_eq!(paths(&empty), ["y"]);
    }

    #[test]
    fn jumping_leaves_skipped_entries_in_place() {
        let mut p = list(5);
        p.next = Some(uid_at(&p, 3));
        assert_eq!(p.advance(all), Some(uid_at(&p, 4)));
        assert_eq!(paths(&p), ["0", "1", "2", "3", "4"]);
        // Nothing before the jump counts as played.
        assert!(p.played.is_empty());
    }

    #[test]
    fn removing_next_reports_it() {
        let mut p = list(3);
        p.next = Some(uid_at(&p, 1));
        assert!(!p.remove(uid_at(&p, 0)));
        assert!(p.remove(p.next.unwrap()));
        assert_eq!(p.next, None);
        assert_eq!(paths(&p), ["2"]);
    }

    #[test]
    fn points_set_and_clear() {
        let mut p = list(1);
        let u = uid_at(&p, 0);
        p.set_start(u, Some(12.0));
        p.set_end(u, Some(200.0));
        assert_eq!(p.items()[0], ("/m/0.flac".into(), Some(12.0), Some(200.0)));
        p.dirty = false;
        p.set_start(u, Some(12.0));
        assert!(!p.dirty, "same value is not a change");
        p.set_start(u, None);
        assert!(p.dirty);
    }

    #[test]
    fn revert_keeps_marks_on_the_same_files() {
        let mut p = list(4);
        let saved = p.items();
        p.next = p.resume_point(all);
        p.advance(all); // 0 on air, 1 next
        p.insert_as_next("/m/x.flac".into()); // x next
        p.remove(uid_at(&p, 4)); // drop 3
        p.revert_to(saved);
        assert_eq!(paths(&p), ["0", "1", "2", "3"]);
        assert!(!p.dirty);
        assert_eq!(p.on_air, Some(uid_at(&p, 0)));
        assert_eq!(p.next, None, "x is not in the saved list");
        assert_eq!(p.name.as_deref(), Some("Sabato"));
    }

    #[test]
    fn resume_point_skips_played_and_on_air() {
        let mut p = list(4);
        p.next = p.resume_point(all);
        p.advance(all);
        p.advance(all); // 0 played, 1 on air, 2 next
        assert_eq!(p.resume_point(all), Some(uid_at(&p, 2)));
    }

    #[test]
    fn survives_a_round_trip_through_toml() {
        let mut p = list(2);
        p.set_start(uid_at(&p, 1), Some(3.5));
        p.next = p.resume_point(all);
        let text = toml::to_string(&p).unwrap();
        let back: Playlist = toml::from_str(&text).unwrap();
        assert_eq!(back, p);
        let mut back = back;
        assert_ne!(back.push("/m/z.flac".into()), uid_at(&p, 0));
    }
}
