//! The workspace side of the setlist: analysing tracks before they reach a
//! deck, keeping deck B fed from the open setlist, and saving it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Context, Window};

use super::{Deck, Dialog, Then, WAVE_BARS, Workspace};
use crate::audio::analysis::{self, Analysis};
use crate::audio::engine::Command;
use crate::audio::transport::{Slot, Which};
use crate::playlist::Playlist;
use crate::settings;

/// Called once a track's analysis is ready (or failed).
pub type Waiter =
    Box<dyn FnOnce(&mut Workspace, Result<Arc<Analysis>, String>, &mut Context<Workspace>)>;

/// Analyses run in the background, a couple at a time: a long setlist
/// opened at once should not take every core from the audio.
const PARALLEL_ANALYSES: usize = 2;

fn slot_ix(deck: Deck) -> usize {
    match deck {
        Deck::OnAir => 0,
        Deck::Next => 1,
    }
}

/// How an entry shows in the setlist panel.
pub struct EntryView {
    pub title: String,
    pub artist: String,
    pub start: f32,
    pub end: f32,
    pub manual_start: bool,
    pub manual_end: bool,
    pub missing: bool,
}

impl Workspace {
    // Analysis

    /// Runs `waiter` with the track's analysis: at once when it is known,
    /// else when a background decode has produced it. `urgent` puts it
    /// ahead of the setlist's prefetching.
    pub(crate) fn with_analysis(
        &mut self,
        path: PathBuf,
        urgent: bool,
        waiter: Waiter,
        cx: &mut Context<Self>,
    ) {
        if let Some(a) = self.analysis_of(&path) {
            waiter(self, Ok(a), cx);
            return;
        }
        self.analysis_waiters
            .entry(path.clone())
            .or_default()
            .push(waiter);
        if self.analysis_inflight.insert(path.clone()) {
            if urgent {
                self.analysis_queue.push_front(path);
            } else {
                self.analysis_queue.push_back(path);
            }
        } else if urgent && let Some(i) = self.analysis_queue.iter().position(|p| *p == path) {
            let p = self.analysis_queue.remove(i).unwrap();
            self.analysis_queue.push_front(p);
        }
        self.pump_analyses(cx);
    }

    /// Known in memory or in the database.
    pub(crate) fn analysis_of(&mut self, path: &Path) -> Option<Arc<Analysis>> {
        if let Some(a) = self.analyses.get(path) {
            return Some(a.clone());
        }
        let a = Arc::new(self.store.as_ref()?.analysis(path)?);
        self.analyses.insert(path.to_path_buf(), a.clone());
        Some(a)
    }

    fn pump_analyses(&mut self, cx: &mut Context<Self>) {
        while self.analysing < PARALLEL_ANALYSES {
            let Some(path) = self.analysis_queue.pop_front() else {
                return;
            };
            self.analysing += 1;
            let task = cx.background_executor().spawn({
                let path = path.clone();
                async move { analysis::analyse(&path, WAVE_BARS) }
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    this.analysing -= 1;
                    this.analysis_inflight.remove(&path);
                    let result = match result {
                        Ok(a) => {
                            if let Some(store) = &this.store {
                                // Only a cache: losing it costs a decode.
                                let _ = store.put_analysis(&path, &a);
                            }
                            let a = Arc::new(a);
                            this.analyses.insert(path.clone(), a.clone());
                            Ok(a)
                        }
                        Err(e) => Err(format!("{}: {e:#}", path.display())),
                    };
                    for waiter in this.analysis_waiters.remove(&path).unwrap_or_default() {
                        waiter(this, result.clone(), cx);
                    }
                    this.pump_analyses(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// The waveform of a deck's track, if analysed.
    pub(crate) fn peaks_of(&self, id: i64) -> Option<Arc<Analysis>> {
        let path = &self.track(id)?.path;
        self.analyses.get(path).cloned()
    }

    // Loading

    pub(crate) fn deck_a_free(&self) -> bool {
        let s = &self.snapshot;
        s.incoming.is_none() && !(s.playing && s.on_air.is_some())
    }

    /// Deck B (or A) is waiting for an analysis before the track goes in.
    pub(crate) fn loading(&self, deck: Deck) -> bool {
        self.pending_load[slot_ix(deck)].is_some()
    }

    pub(crate) fn load_entry(&mut self, uid: u64, deck: Deck, cx: &mut Context<Self>) {
        let Some(path) = self.playlist.get(uid).map(|e| e.path.clone()) else {
            return;
        };
        self.load_path(path, Some(uid), deck, cx);
    }

    /// Analyses the file, then puts it in the deck with the entry's start
    /// and end (or the sound's own). A later load into the same deck wins.
    pub(crate) fn load_path(
        &mut self,
        path: PathBuf,
        entry: Option<u64>,
        deck: Deck,
        cx: &mut Context<Self>,
    ) {
        self.load_seq += 1;
        let seq = self.load_seq;
        self.pending_load[slot_ix(deck)] = Some(seq);
        cx.notify();
        let file = path.clone();
        self.with_analysis(
            path,
            true,
            Box::new(move |this, result, cx| {
                if this.pending_load[slot_ix(deck)] != Some(seq) {
                    return;
                }
                this.pending_load[slot_ix(deck)] = None;
                let a = match result {
                    Ok(a) => a,
                    Err(e) => {
                        this.alert(format!("Impossibile leggere il brano: {e}"));
                        return;
                    }
                };
                let Some(id) = this.by_path.get(&file).map(|&i| this.tracks[i].id) else {
                    this.alert(format!(
                        "«{}» non è nella libreria: aggiorna la libreria.",
                        file.display()
                    ));
                    return;
                };
                let e = entry.and_then(|u| this.playlist.get(u));
                let start = e
                    .and_then(|e| e.start)
                    .unwrap_or(a.sound_start)
                    .clamp(0.0, (a.duration - 0.5).max(0.0));
                let end = e
                    .and_then(|e| e.end)
                    .unwrap_or(a.sound_end)
                    .clamp((start + 0.5).min(a.duration), a.duration);
                let slot = Slot {
                    id,
                    path: file.clone(),
                    duration: a.duration,
                    start,
                    end,
                    gain: a.gain(),
                };
                this.engine.send(match deck {
                    Deck::Next => Command::LoadNext(slot),
                    Deck::OnAir => Command::LoadOnAir(slot),
                });
                cx.notify();
            }),
            cx,
        );
    }

    /// The engine took deck B for the speakers: the setlist moves on and
    /// the entry after it is loaded into B.
    pub(crate) fn next_went_on_air(&mut self, id: i64, cx: &mut Context<Self>) {
        let from_setlist = self.playlist.next.is_some_and(|n| {
            self.playlist
                .get(n)
                .and_then(|e| self.by_path.get(&e.path))
                .is_some_and(|&i| self.tracks[i].id == id)
        });
        if from_setlist && !self.loading(Deck::Next) {
            let by_path = &self.by_path;
            if let Some(next) = self.playlist.advance(|p| by_path.contains_key(p)) {
                self.load_entry(next, Deck::Next, cx);
            }
        } else {
            // Something else went out (B was being replaced): the setlist
            // keeps its next, which is still on its way into B.
            self.playlist.leave_air();
        }
        self.playlist_changed(cx);
    }

    // Start and end points

    pub(crate) fn set_start(&mut self, secs: f32, cx: &mut Context<Self>) {
        let Some(next) = self.snapshot.next.clone() else {
            return;
        };
        let secs = secs.clamp(0.0, (next.duration - 0.5).max(0.0));
        self.engine.send(Command::SetStart(secs));
        if let Some(uid) = self.playlist.next
            && !self.loading(Deck::Next)
        {
            self.playlist.set_start(uid, Some(secs));
            // The engine keeps the end after the start; so does the entry.
            if next.end < secs + 0.5 {
                self.playlist
                    .set_end(uid, Some((secs + 0.5).min(next.duration)));
            }
            self.playlist_changed(cx);
        }
    }

    pub(crate) fn set_end(&mut self, deck: Deck, secs: f32, cx: &mut Context<Self>) {
        let s = &self.snapshot;
        let (info, which, uid) = match deck {
            // A running crossfade is not moved.
            Deck::OnAir if s.incoming.is_some() => return,
            Deck::OnAir => (s.on_air.clone(), Which::OnAir, self.playlist.on_air),
            Deck::Next => (s.next.clone(), Which::Next, self.playlist.next),
        };
        let Some(info) = info else { return };
        let secs = secs.clamp((info.start + 0.5).min(info.duration), info.duration);
        self.engine.send(Command::SetEnd(which, secs));
        if let Some(uid) = uid
            && !self.loading(deck)
        {
            self.playlist.set_end(uid, Some(secs));
            self.playlist_changed(cx);
        }
    }

    // The setlist

    /// Saves tonight's copy, so a crash or a quit loses nothing, and starts
    /// analysing its tracks so their lengths and points show up.
    pub(crate) fn playlist_changed(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = settings::save_session(&self.playlist) {
            self.alert(format!("Impossibile salvare la scaletta di stasera: {e}"));
        }
        let paths: Vec<PathBuf> = self
            .playlist
            .entries
            .iter()
            .map(|e| e.path.clone())
            .filter(|p| self.by_path.contains_key(p) && !self.analyses.contains_key(p))
            .collect();
        for path in paths {
            if !self.analysis_inflight.contains(&path) {
                self.with_analysis(path, false, Box::new(|_, _, _| {}), cx);
            }
        }
        cx.notify();
    }

    /// At launch: tonight's setlist comes back as it was. Nothing plays;
    /// deck B gets the entry where it had got to.
    pub(crate) fn resume_session(&mut self, cx: &mut Context<Self>) {
        self.playlist.leave_air();
        self.playlist.next = None;
        self.feed_next(cx);
        self.playlist_changed(cx);
    }

    /// With nothing marked next, loads where the setlist carries on.
    fn feed_next(&mut self, cx: &mut Context<Self>) {
        if self.playlist.next.is_some() {
            return;
        }
        let by_path = &self.by_path;
        self.playlist.next = self.playlist.resume_point(|p| by_path.contains_key(p));
        match self.playlist.next {
            Some(uid) => self.load_entry(uid, Deck::Next, cx),
            None => {
                self.pending_load[slot_ix(Deck::Next)] = None;
                if self.snapshot.next.is_some() {
                    self.engine.send(Command::ClearNext);
                }
            }
        }
    }

    pub(crate) fn entry_view(&self, uid: u64) -> Option<EntryView> {
        let e = self.playlist.get(uid)?;
        let track = self.by_path.get(&e.path).map(|&i| &self.tracks[i]);
        let a = self.analyses.get(&e.path);
        let duration = a.map_or_else(|| track.map_or(0.0, |t| t.duration_secs), |a| a.duration);
        let start = e.start.or(a.map(|a| a.sound_start)).unwrap_or(0.0);
        let end = e.end.or(a.map(|a| a.sound_end)).unwrap_or(duration);
        let stem = || {
            e.path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        Some(EntryView {
            title: track.map_or_else(stem, |t| t.title.clone()),
            artist: track.map(|t| t.artist.clone()).unwrap_or_default(),
            start,
            end,
            manual_start: e.start.is_some(),
            manual_end: e.end.is_some(),
            missing: track.is_none(),
        })
    }

    /// Sum of every entry's start-to-end length.
    pub(crate) fn setlist_length(&self) -> f32 {
        self.playlist
            .entries
            .iter()
            .filter_map(|e| self.entry_view(e.uid))
            .map(|v| (v.end - v.start).max(0.0))
            .sum()
    }

    pub(crate) fn add_selected_to_setlist(&mut self, cx: &mut Context<Self>) {
        let Some(&ix) = self.cursor.and_then(|c| self.results.get(c)) else {
            return;
        };
        self.add_to_setlist(ix, cx);
    }

    pub(crate) fn add_to_setlist(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(path) = self.tracks.get(ix).map(|t| t.path.clone()) else {
            return;
        };
        self.playlist.push(path);
        // An empty deck B picks it up: the queue carries on.
        self.feed_next(cx);
        self.playlist_changed(cx);
    }

    pub(crate) fn setlist_select(&mut self, uid: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.setlist_cursor = Some(uid);
        window.focus(&self.setlist_focus, cx);
        cx.notify();
    }

    pub(crate) fn setlist_move_cursor(&mut self, delta: isize, cx: &mut Context<Self>) {
        let entries = &self.playlist.entries;
        if entries.is_empty() {
            return;
        }
        let at = self
            .setlist_cursor
            .and_then(|u| self.playlist.index_of(u))
            .map_or(0, |i| {
                (i as isize + delta).clamp(0, entries.len() as isize - 1) as usize
            });
        self.setlist_cursor = Some(entries[at].uid);
        cx.notify();
    }

    /// The entry becomes the next one and goes into deck B. Entries
    /// jumped over stay where they are.
    pub(crate) fn setlist_load(&mut self, uid: u64, cx: &mut Context<Self>) {
        let Some(e) = self.playlist.get(uid) else {
            return;
        };
        if !self.by_path.contains_key(&e.path) {
            self.alert("Questo brano non è più nella libreria.");
            cx.notify();
            return;
        }
        self.playlist.next = Some(uid);
        self.load_entry(uid, Deck::Next, cx);
        self.playlist_changed(cx);
    }

    pub(crate) fn setlist_load_on_air(&mut self, uid: u64, cx: &mut Context<Self>) {
        if !self.deck_a_free() {
            self.alert(
                "Il deck A sta suonando: mettilo in pausa o fermalo prima di caricarci un brano.",
            );
            cx.notify();
            return;
        }
        if self.playlist.get(uid).is_none() {
            return;
        }
        self.playlist.leave_air();
        self.playlist.on_air = Some(uid);
        self.load_entry(uid, Deck::OnAir, cx);
        if self.playlist.next.is_none_or(|n| n == uid) {
            let by_path = &self.by_path;
            self.playlist.next = self
                .playlist
                .following(Some(uid), |p| by_path.contains_key(p));
            match self.playlist.next {
                Some(n) => self.load_entry(n, Deck::Next, cx),
                None => self.engine.send(Command::ClearNext),
            }
        }
        self.playlist_changed(cx);
    }

    pub(crate) fn setlist_remove(&mut self, uid: u64, cx: &mut Context<Self>) {
        let after = self
            .playlist
            .index_of(uid)
            .and_then(|i| {
                self.playlist
                    .entries
                    .get(i + 1)
                    .or(i.checked_sub(1).and_then(|j| self.playlist.entries.get(j)))
            })
            .map(|e| e.uid);
        if self.playlist.remove(uid) {
            self.engine.send(Command::ClearNext);
            self.pending_load[slot_ix(Deck::Next)] = None;
            self.feed_next(cx);
        }
        if self.setlist_cursor == Some(uid) {
            self.setlist_cursor = after;
        }
        self.playlist_changed(cx);
    }

    pub(crate) fn setlist_shift(&mut self, uid: u64, by: isize, cx: &mut Context<Self>) {
        self.playlist.shift(uid, by);
        self.playlist_changed(cx);
    }

    /// Back to the analysis' start and end.
    pub(crate) fn setlist_clear_points(&mut self, uid: u64, cx: &mut Context<Self>) {
        self.playlist.set_start(uid, None);
        self.playlist.set_end(uid, None);
        if self.playlist.next == Some(uid) {
            self.load_entry(uid, Deck::Next, cx);
        } else if self.playlist.on_air == Some(uid)
            && let Some(a) = self
                .playlist
                .get(uid)
                .and_then(|e| self.analyses.get(&e.path))
        {
            self.engine.send(Command::SetEnd(Which::OnAir, a.sound_end));
        }
        self.playlist_changed(cx);
    }

    pub(crate) fn with_setlist_cursor(&mut self, f: impl FnOnce(&mut Self, u64)) {
        if let Some(uid) = self.setlist_cursor {
            f(self, uid);
        }
    }

    // Opening, saving

    pub(crate) fn refresh_saved_lists(&mut self) {
        if let Some(store) = &self.store {
            match store.playlists() {
                Ok(lists) => self.saved_lists = lists,
                Err(e) => self.alert(format!("Impossibile leggere le scalette salvate: {e}")),
            }
        }
    }

    pub(crate) fn show_dialog(
        &mut self,
        dialog: Dialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_error = None;
        match &dialog {
            Dialog::Name { rename, .. } => {
                let current = match rename {
                    Some(id) => self
                        .saved_lists
                        .iter()
                        .find(|l| l.id == *id)
                        .map(|l| l.name.clone()),
                    None => self.playlist.name.clone(),
                }
                .unwrap_or_default();
                self.name_input.update(cx, |input, cx| {
                    input.set_value(current, window, cx);
                    input.focus(window, cx);
                });
            }
            Dialog::Open => self.refresh_saved_lists(),
            _ => {}
        }
        self.dialog = Some(dialog);
        cx.notify();
    }

    pub(crate) fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        self.dialog_error = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Replacing the open setlist asks first when it has unsaved changes.
    pub(crate) fn replace_setlist(
        &mut self,
        then: Then,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.playlist.dirty && !self.playlist.entries.is_empty() {
            self.show_dialog(Dialog::Unsaved(then), window, cx);
        } else {
            self.close_dialog(window, cx);
            self.run_then(then, cx);
        }
    }

    pub(crate) fn run_then(&mut self, then: Then, cx: &mut Context<Self>) {
        let fresh = match then {
            Then::New => Playlist::new(),
            Then::Open(id) => {
                let Some(store) = &self.store else { return };
                match store.load_playlist(id) {
                    Ok((name, items)) => Playlist::opened(id, name, items),
                    Err(e) => {
                        self.alert(format!("Impossibile aprire la scaletta: {e}"));
                        cx.notify();
                        return;
                    }
                }
            }
        };
        // Deck A plays on untouched; deck B held the old setlist's next.
        self.playlist = fresh;
        self.setlist_cursor = None;
        self.pending_load[slot_ix(Deck::Next)] = None;
        self.feed_next(cx);
        self.playlist_changed(cx);
    }

    /// Saves over the open setlist, or asks for a name when it has none.
    pub(crate) fn save_setlist(
        &mut self,
        then: Option<Then>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.playlist.name.clone() else {
            self.show_dialog(Dialog::Name { rename: None, then }, window, cx);
            return;
        };
        let id = self.playlist.id;
        if self.write_setlist(id, &name, cx) {
            self.close_dialog(window, cx);
            if let Some(then) = then {
                self.run_then(then, cx);
            }
        }
    }

    fn write_setlist(&mut self, id: Option<i64>, name: &str, cx: &mut Context<Self>) -> bool {
        let Some(store) = &mut self.store else {
            return false;
        };
        match store.save_playlist(id, name, &self.playlist.items()) {
            Ok(id) => {
                self.playlist.mark_saved(id, name.trim().to_string());
                self.playlist_changed(cx);
                true
            }
            Err(e) => {
                self.dialog_error = Some(format!("{e:#}"));
                if self.dialog.is_none() {
                    self.alert(format!("Scaletta non salvata: {e:#}"));
                }
                cx.notify();
                false
            }
        }
    }

    /// Enter or OK in the name dialog.
    pub(crate) fn confirm_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Name { rename, then }) = self.dialog.clone() else {
            return;
        };
        let name = self.name_input.read(cx).value().trim().to_string();
        match rename {
            Some(id) => {
                let Some(store) = &self.store else { return };
                match store.rename_playlist(id, &name) {
                    Ok(()) => {
                        if self.playlist.id == Some(id) {
                            self.playlist.name = Some(name);
                            self.playlist_changed(cx);
                        }
                        self.show_dialog(Dialog::Open, window, cx);
                    }
                    Err(e) => {
                        self.dialog_error = Some(format!("{e:#}"));
                        cx.notify();
                    }
                }
            }
            None => {
                if self.write_setlist(None, &name, cx) {
                    self.close_dialog(window, cx);
                    if let Some(then) = then {
                        self.run_then(then, cx);
                    }
                }
            }
        }
    }

    pub(crate) fn revert_setlist(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(store)) = (self.playlist.id, &self.store) else {
            return;
        };
        match store.load_playlist(id) {
            Ok((_, items)) => {
                let had_next = self.playlist.next.is_some();
                self.playlist.revert_to(items);
                if had_next && self.playlist.next.is_none() {
                    self.pending_load[slot_ix(Deck::Next)] = None;
                    self.feed_next(cx);
                }
                self.playlist_changed(cx);
            }
            Err(e) => {
                self.alert(format!("Impossibile ripristinare la scaletta: {e}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn delete_setlist(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = &self.store else { return };
        if let Err(e) = store.delete_playlist(id) {
            self.alert(format!("Impossibile eliminare la scaletta: {e}"));
        }
        if self.playlist.id == Some(id) {
            // Still open: it stays as an unsaved copy.
            self.playlist.id = None;
            self.playlist.dirty = true;
            self.playlist_changed(cx);
        }
        self.show_dialog(Dialog::Open, window, cx);
    }
}
