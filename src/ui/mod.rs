mod render;
pub mod theme;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::Receiver;
use gpui::{
    AppContext, Context, Entity, FocusHandle, PathPromptOptions, ScrollStrategy, SharedString,
    UniformListScrollHandle, Window,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::resizable::ResizableState;
use gpui_component::slider::{SliderEvent, SliderState};

use crate::audio::engine::{Command, Engine, EngineEvent, Role, Snapshot};
use crate::audio::transport::Slot;
use crate::audio::{output, waveform};
use crate::library::search::Index;
use crate::library::tree::{self, FolderNode};
use crate::library::{Library, ScanReport, Track};
use crate::settings::{self, Settings};
use theme::{Appearance, Theme};

pub const WAVE_BARS: usize = 160;
const POLL_EVERY: Duration = Duration::from_millis(40);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Library,
    Audio,
    Mix,
    Look,
    Keys,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 5] = [
        Self::Library,
        Self::Audio,
        Self::Mix,
        Self::Look,
        Self::Keys,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "LIBRERIA",
            Self::Audio => "AUDIO",
            Self::Mix => "MIX",
            Self::Look => "ASPETTO",
            Self::Keys => "TASTI",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Deck {
    OnAir,
    Next,
}

pub struct Workspace {
    pub(crate) focus: FocusHandle,
    theme: Theme,
    settings: Settings,
    /// Taken out while a rescan runs on a background thread.
    library: Option<Library>,
    scanning: bool,
    tracks: Arc<Vec<Track>>,
    by_id: HashMap<i64, usize>,
    index: Arc<Index>,
    tree: Option<FolderNode>,
    expanded: HashSet<PathBuf>,
    /// The folder the list and the search are limited to; None is the root.
    scope: Option<PathBuf>,
    search: Entity<InputState>,
    results: Vec<usize>,
    cursor: Option<usize>,
    list_scroll: UniformListScrollHandle,
    engine: Engine,
    snapshot: Snapshot,
    preview_pos: Option<f32>,
    waves: HashMap<i64, Arc<Vec<f32>>>,
    waves_pending: HashSet<i64>,
    alerts: Vec<SharedString>,
    show_settings: bool,
    settings_tab: SettingsTab,
    devices: Vec<String>,
    split: Entity<ResizableState>,
    master_volume: Entity<SliderState>,
    cue_volume: Entity<SliderState>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut settings = settings::load();
        if settings.master_device.is_none() {
            settings.master_device = output::default_device();
            let _ = settings::save(&settings);
        }
        let (engine, events) = Engine::start(settings.crossfade_secs);
        engine.send(Command::SetDevices {
            master: settings.master_device.clone(),
            cue: settings.cue_device.clone(),
        });

        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Cerca artista, titolo, album…"));
        cx.subscribe_in(
            &search,
            window,
            |this, _, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => this.run_query(cx),
                InputEvent::PressEnter {
                    secondary: true, ..
                } => this.engine.send(Command::Crossfade),
                InputEvent::PressEnter { .. } => this.load_selected(cx),
                _ => {}
            },
        )
        .detach();
        search.update(cx, |s, cx| s.focus(window, cx));
        let theme = theme::find(
            settings.theme.as_deref(),
            Appearance::of_system(window.appearance()),
        );
        theme.apply_to_components(cx);
        // With no theme chosen, follow the system between light and dark.
        cx.observe_window_appearance(window, |this: &mut Self, window, cx| {
            if this.settings.theme.is_none() {
                this.theme = theme::find(None, Appearance::of_system(window.appearance()));
                this.theme.apply_to_components(cx);
                cx.notify();
            }
        })
        .detach();

        engine.send(Command::SetVolume(Role::Master, settings.master_volume));
        engine.send(Command::SetVolume(Role::Cue, settings.cue_volume));
        let master_volume = Self::volume_slider(Role::Master, settings.master_volume, cx);
        let cue_volume = Self::volume_slider(Role::Cue, settings.cue_volume, cx);

        let mut alerts = Vec::new();
        let library = match Library::open(&settings::support_dir().join("library.sqlite")) {
            Ok(lib) => Some(lib),
            Err(e) => {
                alerts.push(format!("Impossibile aprire l'indice della libreria: {e}").into());
                None
            }
        };

        let mut this = Self {
            focus: cx.focus_handle(),
            theme,
            settings,
            library,
            scanning: false,
            tracks: Arc::default(),
            by_id: HashMap::new(),
            index: Arc::new(Index::new(&[])),
            tree: None,
            expanded: HashSet::new(),
            scope: None,
            search,
            results: Vec::new(),
            cursor: None,
            list_scroll: UniformListScrollHandle::new(),
            engine,
            snapshot: Snapshot::default(),
            preview_pos: None,
            waves: HashMap::new(),
            waves_pending: HashSet::new(),
            alerts,
            show_settings: false,
            settings_tab: SettingsTab::Library,
            devices: output::list_devices(),
            split: cx.new(|_| ResizableState::default()),
            master_volume,
            cue_volume,
        };
        this.refresh_tracks(cx);
        this.rescan(cx);
        this.poll_engine(events, cx);
        this
    }

    fn poll_engine(&mut self, events: Receiver<EngineEvent>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_EVERY).await;
                let batch: Vec<EngineEvent> = events.try_iter().collect();
                if batch.is_empty() {
                    continue;
                }
                if this
                    .update(cx, |this, cx| this.on_events(batch, cx))
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();
    }

    fn on_events(&mut self, batch: Vec<EngineEvent>, cx: &mut Context<Self>) {
        for event in batch {
            match event {
                EngineEvent::State(s) => self.snapshot = s,
                EngineEvent::PreviewPos(p) => self.preview_pos = p,
                EngineEvent::DeviceLost(Role::Cue) => {
                    self.alert("Cuffia scollegata: anteprima fermata. Le casse continuano.")
                }
                EngineEvent::DeviceLost(Role::Master) => {
                    self.alert("Uscita casse scollegata: riproduzione fermata.")
                }
                EngineEvent::DeviceMissing(role) => {
                    let (what, name) = match role {
                        Role::Master => ("Casse", &self.settings.master_device),
                        Role::Cue => ("Cuffia", &self.settings.cue_device),
                    };
                    let msg = match name {
                        Some(n) => format!(
                            "{what}: «{n}» non è collegato al Mac. Collegalo (per il Bluetooth: \
                             menu Bluetooth di macOS) e riprova, oppure scegli un'altra uscita in \
                             Impostazioni › Audio."
                        ),
                        None => format!(
                            "{what}: nessuna uscita scelta. Sceglila in Impostazioni › Audio."
                        ),
                    };
                    self.alert(msg)
                }
                EngineEvent::Error(e) => self.alert(e),
            }
        }
        if !self.snapshot.previewing {
            self.preview_pos = None;
        }
        let ids: Vec<i64> = [
            &self.snapshot.on_air,
            &self.snapshot.next,
            &self.snapshot.incoming,
        ]
        .into_iter()
        .flatten()
        .map(|d| d.id)
        .collect();
        for id in ids {
            self.ensure_wave(id, cx);
        }
        cx.notify();
    }

    pub(crate) fn alert(&mut self, text: impl Into<SharedString>) {
        let text = text.into();
        if self.alerts.last() != Some(&text) {
            self.alerts.push(text);
        }
        if self.alerts.len() > 4 {
            self.alerts.remove(0);
        }
    }

    fn ensure_wave(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.waves.contains_key(&id) || !self.waves_pending.insert(id) {
            return;
        }
        let Some(path) = self.track(id).map(|t| t.path.clone()) else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { waveform::peaks(&path, WAVE_BARS) });
        cx.spawn(async move |this, cx| {
            let peaks = task.await.unwrap_or_else(|_| vec![0.0; WAVE_BARS]);
            let _ = this.update(cx, |this, cx| {
                this.waves_pending.remove(&id);
                this.waves.insert(id, Arc::new(peaks));
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn track(&self, id: i64) -> Option<&Track> {
        self.by_id.get(&id).map(|&i| &self.tracks[i])
    }

    // Library

    pub(crate) fn rescan(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.settings.library_root.clone() else {
            return;
        };
        let Some(mut library) = self.library.take() else {
            return;
        };
        self.scanning = true;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            let report = library.rescan(&root);
            (library, report)
        });
        cx.spawn(async move |this, cx| {
            let (library, report) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.library = Some(library);
                this.scanning = false;
                match report {
                    Ok(ScanReport::RootMissing) => this.alert(
                        "Cartella della musica non raggiungibile: mostro l'ultimo indice salvato.",
                    ),
                    Ok(ScanReport::Done { .. }) => {}
                    Err(e) => this.alert(format!("Aggiornamento della libreria non riuscito: {e}")),
                }
                this.refresh_tracks(cx);
            });
        })
        .detach();
    }

    fn refresh_tracks(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &self.library else { return };
        let root = self.settings.library_root.clone();
        // Rows from a previous library folder stay in the index until the
        // next rescan of the new one; never show them.
        let tracks: Vec<Track> = library
            .tracks()
            .iter()
            .filter(|t| root.as_ref().is_some_and(|r| t.path.starts_with(r)))
            .cloned()
            .collect();
        self.by_id = tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
        self.index = Arc::new(Index::new(&tracks));
        self.tree = root.as_ref().map(|r| tree::build(r, &tracks));
        if let Some(r) = &root {
            self.expanded.insert(r.clone());
        }
        self.tracks = Arc::new(tracks);
        if self
            .scope
            .as_ref()
            .is_some_and(|s| !self.tracks.iter().any(|t| t.path.starts_with(s)))
        {
            self.scope = None;
        }
        self.run_query(cx);
    }

    fn run_query(&mut self, cx: &mut Context<Self>) {
        let q = self.search.read(cx).value().to_string();
        self.results = self.index.query(&self.tracks, &q, self.scope.as_deref());
        self.cursor = (!self.results.is_empty()).then_some(0);
        self.list_scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    pub(crate) fn select_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let is_root = self.settings.library_root.as_ref() == Some(&path);
        if self.scope.as_ref() == Some(&path) || (is_root && self.scope.is_none()) {
            // A second click on the selected folder opens or closes it.
            if !self.expanded.remove(&path) {
                self.expanded.insert(path);
            }
        } else {
            self.expanded.insert(path.clone());
            self.scope = (!is_root).then_some(path);
        }
        self.run_query(cx);
    }

    pub(crate) fn toggle_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.expanded.remove(&path) {
            self.expanded.insert(path);
        }
        cx.notify();
    }

    pub(crate) fn move_cursor(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        let last = self.results.len() as isize - 1;
        let at = self
            .cursor
            .map_or(0, |c| (c as isize + delta).clamp(0, last)) as usize;
        self.cursor = Some(at);
        self.list_scroll.scroll_to_item(at, ScrollStrategy::Center);
        cx.notify();
    }

    /// Double click and Enter load into deck B; the context menu can pick.
    pub(crate) fn load_row(&mut self, row: usize, deck: Deck, cx: &mut Context<Self>) {
        self.cursor = Some(row);
        self.load_selected_to(deck, cx);
    }

    pub(crate) fn load_selected(&mut self, cx: &mut Context<Self>) {
        self.load_selected_to(Deck::Next, cx);
    }

    fn load_selected_to(&mut self, deck: Deck, cx: &mut Context<Self>) {
        let Some(&ix) = self.cursor.and_then(|c| self.results.get(c)) else {
            return;
        };
        let t = &self.tracks[ix];
        let slot = Slot {
            id: t.id,
            path: t.path.clone(),
            duration: t.duration_secs,
            start: 0.0,
        };
        self.engine.send(match deck {
            Deck::OnAir => Command::LoadOnAir(slot),
            Deck::Next => Command::LoadNext(slot),
        });
        self.ensure_wave(t.id, cx);
        cx.notify();
    }

    // Decks

    pub(crate) fn send(&self, cmd: Command) {
        self.engine.send(cmd);
    }

    pub(crate) fn play_pause(&mut self) {
        let s = &self.snapshot;
        if s.on_air.is_some() && s.playing {
            self.engine.send(Command::Pause);
        } else {
            self.engine.send(Command::Play);
        }
    }

    pub(crate) fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        if self.snapshot.next.is_none() {
            self.alert("Il deck B è vuoto: carica un brano con doppio clic (o Invio) nella lista, poi premi CUE.");
            cx.notify();
            return;
        }
        self.engine.send(Command::TogglePreview);
    }

    pub(crate) fn nudge(&mut self, by: f32) {
        if let Some(next) = &self.snapshot.next {
            self.engine.send(Command::SetStart(next.start + by));
        }
    }

    /// A click on bar `bar` of a deck's waveform: seek what is on air, or set
    /// where Next will start.
    pub(crate) fn click_wave(&mut self, deck: Deck, bar: usize) {
        let frac = (bar as f32 + 0.5) / WAVE_BARS as f32;
        match deck {
            Deck::OnAir => {
                if let Some(on) = &self.snapshot.on_air {
                    self.engine.send(Command::Seek(on.duration * frac));
                }
            }
            Deck::Next => {
                if let Some(next) = &self.snapshot.next {
                    self.engine.send(Command::SetStart(next.duration * frac));
                }
            }
        }
    }

    // Settings

    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.devices = output::list_devices();
        self.show_settings = true;
        cx.notify();
    }

    pub(crate) fn show_tab(&mut self, tab: SettingsTab, cx: &mut Context<Self>) {
        if tab == SettingsTab::Audio {
            self.devices = output::list_devices();
        }
        self.settings_tab = tab;
        cx.notify();
    }

    pub(crate) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_settings = false;
        self.search.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn set_device(&mut self, role: Role, name: String, cx: &mut Context<Self>) {
        match role {
            Role::Master => self.settings.master_device = Some(name),
            Role::Cue => self.settings.cue_device = Some(name),
        }
        self.save_settings();
        self.engine.send(Command::SetDevices {
            master: self.settings.master_device.clone(),
            cue: self.settings.cue_device.clone(),
        });
        cx.notify();
    }

    pub(crate) fn change_crossfade(&mut self, by: f32, cx: &mut Context<Self>) {
        self.settings.crossfade_secs = (self.settings.crossfade_secs + by).clamp(1.0, 20.0);
        self.engine
            .send(Command::SetCrossfade(self.settings.crossfade_secs));
        self.save_settings();
        cx.notify();
    }

    pub(crate) fn pick_library(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Usa questa cartella".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    this.settings.library_root = Some(path);
                    this.scope = None;
                    this.save_settings();
                    this.refresh_tracks(cx);
                    this.rescan(cx);
                });
            }
        })
        .detach();
    }

    pub(crate) fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        self.settings.theme = Some(theme.name.to_string());
        theme.apply_to_components(cx);
        self.save_settings();
        cx.notify();
    }

    pub(crate) fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        let all = Theme::all();
        let at = all
            .iter()
            .position(|t| t.name == self.theme.name)
            .unwrap_or(0);
        self.set_theme(all[(at + 1) % all.len()], cx);
    }

    /// A 0..100 slider driving one output's volume. It moves the sound while
    /// dragged and is saved when let go.
    fn volume_slider(role: Role, level: f32, cx: &mut Context<Self>) -> Entity<SliderState> {
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(100.)
                .step(1.)
                .default_value(level * 100.)
        });
        cx.subscribe(&slider, move |this, _, event: &SliderEvent, cx| {
            let (level, done) = match event {
                SliderEvent::Change(v) => (v.start() / 100., false),
                SliderEvent::Release(v) => (v.start() / 100., true),
            };
            this.engine.send(Command::SetVolume(role, level));
            match role {
                Role::Master => this.settings.master_volume = level,
                Role::Cue => this.settings.cue_volume = level,
            }
            if done {
                this.save_settings();
            }
            cx.notify();
        })
        .detach();
        slider
    }

    fn save_settings(&mut self) {
        if let Err(e) = settings::save(&self.settings) {
            self.alert(format!("Impossibile salvare le impostazioni: {e}"));
        }
    }
}
