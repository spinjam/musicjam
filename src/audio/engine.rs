//! Two render threads, one per output. The master thread owns the transport
//! and both voices that can reach the speakers; the cue thread owns only the
//! preview, so nothing that happens to the headphones can reach the speakers.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::output::{self, Output, TestTone, Volume};
use super::source::Source;
use super::transport::{Effect, Slot, Transport};

/// Rendered per step: about 10 ms at 48 kHz.
const BLOCK_FRAMES: usize = 512;
const IDLE: Duration = Duration::from_millis(4);
const SNAPSHOT_EVERY: Duration = Duration::from_millis(80);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Master,
    Cue,
}

#[derive(Debug, Clone)]
pub enum Command {
    SetDevices {
        master: Option<String>,
        cue: Option<String>,
    },
    SetCrossfade(f32),
    LoadNext(Slot),
    SetStart(f32),
    TogglePreview,
    Play,
    Pause,
    Stop,
    Crossfade,
    Seek(f32),
    TestTone(Role),
    /// Slider level 0..=1 for one output.
    SetVolume(Role, f32),
    /// From the cue thread: the preview ran to the end of the file.
    PreviewEnded,
}

enum CueCommand {
    SetDevice(Option<String>),
    Start { path: PathBuf, start: f32 },
    Stop,
    TestTone,
    SetVolume(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeckInfo {
    pub id: i64,
    pub pos: f32,
    pub duration: f32,
    pub start: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub on_air: Option<DeckInfo>,
    pub next: Option<DeckInfo>,
    /// The track fading in, while a crossfade runs.
    pub incoming: Option<DeckInfo>,
    pub playing: bool,
    pub previewing: bool,
    pub master_ready: bool,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    State(Snapshot),
    PreviewPos(Option<f32>),
    /// The saved device is not connected.
    DeviceMissing(Role),
    /// The device went away while in use.
    DeviceLost(Role),
    Error(String),
}

pub struct Engine {
    tx: Sender<Command>,
}

impl Engine {
    pub fn start(crossfade_secs: f32) -> (Self, Receiver<EngineEvent>) {
        let (tx, rx) = unbounded();
        let (ev_tx, ev_rx) = unbounded();
        let (cue_tx, cue_rx) = unbounded();
        let master_tx = tx.clone();
        let cue_events = ev_tx.clone();
        thread::Builder::new()
            .name("musicjam-cue".into())
            .spawn(move || cue_thread(cue_rx, cue_events, master_tx))
            .expect("spawn cue thread");
        thread::Builder::new()
            .name("musicjam-master".into())
            .spawn(move || Master::new(crossfade_secs, ev_tx, cue_tx).run(rx))
            .expect("spawn master thread");
        (Self { tx }, ev_rx)
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }
}

struct Master {
    transport: Transport,
    output: Option<Output>,
    device: Option<String>,
    on_air: Option<Source>,
    incoming: Option<Source>,
    tone: Option<TestTone>,
    events: Sender<EngineEvent>,
    cue: Sender<CueCommand>,
    volume: Volume,
    mix: Vec<f32>,
    voice: Vec<f32>,
}

impl Master {
    fn new(crossfade_secs: f32, events: Sender<EngineEvent>, cue: Sender<CueCommand>) -> Self {
        Self {
            transport: Transport::new(crossfade_secs),
            output: None,
            device: None,
            on_air: None,
            incoming: None,
            tone: None,
            events,
            cue,
            volume: Volume::new(1.0),
            mix: vec![0.0; BLOCK_FRAMES * 2],
            voice: vec![0.0; BLOCK_FRAMES * 2],
        }
    }

    fn run(mut self, rx: Receiver<Command>) {
        let mut last_snapshot = Instant::now();
        let mut last_sent = Snapshot::default();
        loop {
            loop {
                match rx.try_recv() {
                    Ok(cmd) => self.handle(cmd),
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => return,
                }
            }
            if self.output.as_ref().is_some_and(Output::is_lost) {
                self.output = None;
                let fx = self.transport.stop();
                self.apply(fx);
                self.emit(EngineEvent::DeviceLost(Role::Master));
            }
            let mut rendered = false;
            while self
                .output
                .as_ref()
                .is_some_and(|o| o.free_frames() >= BLOCK_FRAMES)
            {
                if !self.render_block() {
                    break;
                }
                rendered = true;
            }
            // Only changes go out: an idle player should not keep the UI redrawing.
            if last_snapshot.elapsed() >= SNAPSHOT_EVERY {
                last_snapshot = Instant::now();
                let now = self.snapshot();
                if now != last_sent {
                    last_sent = now.clone();
                    self.emit(EngineEvent::State(now));
                }
            }
            if !rendered {
                thread::sleep(IDLE);
            }
        }
    }

    fn handle(&mut self, cmd: Command) {
        let fx = match cmd {
            Command::SetDevices { master, cue } => {
                let _ = self.cue.send(CueCommand::SetDevice(cue));
                if master != self.device || self.output.is_none() {
                    self.open_device(master);
                }
                vec![]
            }
            Command::SetCrossfade(secs) => {
                self.transport.crossfade_secs = secs.clamp(0.5, 30.0);
                vec![]
            }
            Command::LoadNext(mut slot) => {
                // The real length beats the tags': it decides when the auto
                // crossfade has to start.
                if let Ok(src) = Source::open(&slot.path, 44_100, 0.0)
                    && src.duration_secs() > 0.0
                {
                    slot.duration = src.duration_secs();
                }
                self.transport.load_next(slot)
            }
            Command::SetStart(secs) => self.transport.set_start(secs),
            Command::TogglePreview => self.transport.toggle_preview(),
            Command::Play if self.output.is_some() => self.transport.play(),
            Command::Crossfade if self.output.is_some() => self.transport.crossfade_now(),
            Command::Play | Command::Crossfade => {
                self.emit(EngineEvent::DeviceMissing(Role::Master));
                vec![]
            }
            Command::Pause => {
                self.transport.pause();
                vec![]
            }
            Command::Stop => self.transport.stop(),
            Command::SetVolume(Role::Master, level) => {
                self.volume.set(level);
                vec![]
            }
            Command::SetVolume(Role::Cue, level) => {
                let _ = self.cue.send(CueCommand::SetVolume(level));
                vec![]
            }
            Command::Seek(secs) => self.transport.seek(secs),
            Command::TestTone(Role::Master) => {
                if let Some(out) = &self.output {
                    self.tone = Some(TestTone::new(out.rate));
                } else {
                    self.emit(EngineEvent::DeviceMissing(Role::Master));
                }
                vec![]
            }
            Command::TestTone(Role::Cue) => {
                let _ = self.cue.send(CueCommand::TestTone);
                vec![]
            }
            Command::PreviewEnded => {
                self.transport.previewing = false;
                vec![]
            }
        };
        self.apply(fx);
        self.emit(EngineEvent::State(self.snapshot()));
    }

    fn open_device(&mut self, name: Option<String>) {
        self.output = None;
        self.device = name.clone();
        let fx = self.transport.stop();
        self.apply(fx);
        let Some(name) = name else {
            self.emit(EngineEvent::DeviceMissing(Role::Master));
            return;
        };
        match Output::open(&name) {
            Ok(out) => self.output = Some(out),
            // A device that is simply not plugged in is reported as missing;
            // only a real failure to open one carries its own message.
            Err(_) if !output::list_devices().contains(&name) => {
                self.emit(EngineEvent::DeviceMissing(Role::Master))
            }
            Err(e) => self.emit(EngineEvent::Error(format!("Casse «{name}»: {e}"))),
        }
    }

    fn apply(&mut self, fx: Vec<Effect>) {
        let rate = self.output.as_ref().map_or(48_000, |o| o.rate);
        for effect in fx {
            match effect {
                Effect::StartOnAir { slot, at } => {
                    self.incoming = None;
                    self.on_air = self.open(&slot.path, rate, at);
                }
                Effect::StartIncoming { slot } => {
                    self.incoming = self.open(&slot.path, rate, slot.start)
                }
                Effect::FinishCrossfade => self.on_air = self.incoming.take(),
                Effect::SeekOnAir { at } => {
                    if let Some(on) = &self.transport.on_air {
                        self.on_air = self.open(&on.slot.path.clone(), rate, at);
                    }
                }
                Effect::StopAll => {
                    self.on_air = None;
                    self.incoming = None;
                }
                Effect::StartPreview { slot } => {
                    let _ = self.cue.send(CueCommand::Start {
                        path: slot.path,
                        start: slot.start,
                    });
                }
                Effect::StopPreview => {
                    let _ = self.cue.send(CueCommand::Stop);
                }
            }
        }
    }

    fn open(&self, path: &std::path::Path, rate: u32, at: f32) -> Option<Source> {
        // ponytail: files open on the render thread; the ring's 250 ms covers
        // a local disk. A slow network share would want a loader thread.
        match Source::open(path, rate, at) {
            Ok(s) => Some(s),
            Err(e) => {
                self.emit(EngineEvent::Error(format!("{e:#}")));
                None
            }
        }
    }

    /// Renders one block into the device ring. False when nothing is due.
    fn render_block(&mut self) -> bool {
        let Some(out) = &self.output else {
            return false;
        };
        let rate = out.rate;
        let playing = self.transport.playing && self.on_air.is_some();
        if !playing && self.tone.is_none() {
            return false;
        }
        self.mix.fill(0.0);
        if playing {
            let (g_on, g_in) = self.transport.gains();
            let mut ended = false;
            if let Some(src) = &mut self.on_air {
                let n = src.read(&mut self.voice);
                add(&mut self.mix, &self.voice[..n * 2], g_on);
                ended = n < BLOCK_FRAMES;
            }
            if let Some(src) = &mut self.incoming {
                let n = src.read(&mut self.voice);
                add(&mut self.mix, &self.voice[..n * 2], g_in);
            }
            let mut fx = self.transport.advance(BLOCK_FRAMES as f32 / rate as f32);
            if ended && self.transport.incoming.is_none() && fx.is_empty() {
                fx = self.transport.on_air_ended();
            }
            self.apply(fx);
        }
        // The test tone stays at a fixed level: it must be heard even with
        // the volume down.
        self.volume.apply(&mut self.mix);
        if let Some(tone) = &mut self.tone
            && !tone.mix_into(&mut self.mix)
        {
            self.tone = None;
        }
        if let Some(out) = &mut self.output {
            out.push(&self.mix);
        }
        true
    }

    fn snapshot(&self) -> Snapshot {
        let t = &self.transport;
        Snapshot {
            on_air: t.on_air.as_ref().map(|o| DeckInfo {
                id: o.slot.id,
                pos: o.pos,
                duration: o.slot.duration,
                start: o.slot.start,
            }),
            next: t.next.as_ref().map(|s| DeckInfo {
                id: s.id,
                pos: s.start,
                duration: s.duration,
                start: s.start,
            }),
            incoming: t.incoming.as_ref().map(|i| DeckInfo {
                id: i.slot.id,
                pos: i.slot.start + i.elapsed,
                duration: i.slot.duration,
                start: i.slot.start,
            }),
            playing: t.playing,
            previewing: t.previewing,
            master_ready: self.output.is_some(),
        }
    }

    fn emit(&self, ev: EngineEvent) {
        let _ = self.events.send(ev);
    }
}

fn add(mix: &mut [f32], voice: &[f32], gain: f32) {
    for (m, v) in mix.iter_mut().zip(voice) {
        *m += v * gain;
    }
}

fn cue_thread(rx: Receiver<CueCommand>, events: Sender<EngineEvent>, master: Sender<Command>) {
    let mut output: Option<Output> = None;
    let mut preview: Option<(Source, f32)> = None;
    let mut tone: Option<TestTone> = None;
    let mut volume = Volume::new(1.0);
    let mut buf = vec![0.0f32; BLOCK_FRAMES * 2];
    let mut last_pos = Instant::now();
    loop {
        let cmd = if preview.is_none() && tone.is_none() {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            }
        } else {
            match rx.try_recv() {
                Ok(c) => Some(c),
                Err(crossbeam_channel::TryRecvError::Empty) => None,
                Err(crossbeam_channel::TryRecvError::Disconnected) => return,
            }
        };
        match cmd {
            Some(CueCommand::SetDevice(name)) => {
                output = None;
                preview = None;
                match name.map(|n| Output::open(&n)) {
                    Some(Ok(out)) => output = Some(out),
                    Some(Err(_)) | None => {
                        let _ = events.send(EngineEvent::DeviceMissing(Role::Cue));
                    }
                }
            }
            Some(CueCommand::Start { path, start }) => match &output {
                Some(out) => match Source::open(&path, out.rate, start) {
                    Ok(src) => preview = Some((src, start)),
                    Err(e) => {
                        let _ = events.send(EngineEvent::Error(format!("{e:#}")));
                        let _ = master.send(Command::PreviewEnded);
                    }
                },
                None => {
                    let _ = events.send(EngineEvent::DeviceMissing(Role::Cue));
                    let _ = master.send(Command::PreviewEnded);
                }
            },
            Some(CueCommand::Stop) => {
                preview = None;
                let _ = events.send(EngineEvent::PreviewPos(None));
            }
            Some(CueCommand::SetVolume(level)) => volume.set(level),
            Some(CueCommand::TestTone) => match &output {
                Some(out) => tone = Some(TestTone::new(out.rate)),
                None => {
                    let _ = events.send(EngineEvent::DeviceMissing(Role::Cue));
                }
            },
            None => {}
        }

        let Some(out) = &mut output else { continue };
        if out.is_lost() {
            output = None;
            if preview.take().is_some() {
                let _ = master.send(Command::PreviewEnded);
            }
            tone = None;
            let _ = events.send(EngineEvent::DeviceLost(Role::Cue));
            continue;
        }
        let mut rendered = false;
        while (preview.is_some() || tone.is_some()) && out.free_frames() >= BLOCK_FRAMES {
            buf.fill(0.0);
            if let Some((src, pos)) = &mut preview {
                let n = src.read(&mut buf);
                *pos += n as f32 / out.rate as f32;
                if n < BLOCK_FRAMES {
                    preview = None;
                    let _ = master.send(Command::PreviewEnded);
                    let _ = events.send(EngineEvent::PreviewPos(None));
                }
            }
            volume.apply(&mut buf);
            if let Some(t) = &mut tone
                && !t.mix_into(&mut buf)
            {
                tone = None;
            }
            out.push(&buf);
            rendered = true;
        }
        if let Some((_, pos)) = &preview
            && last_pos.elapsed() >= SNAPSHOT_EVERY
        {
            last_pos = Instant::now();
            let _ = events.send(EngineEvent::PreviewPos(Some(*pos)));
        }
        if !rendered {
            thread::sleep(IDLE);
        }
    }
}
