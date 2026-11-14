//! What plays where and when, without any audio: the engine feeds it elapsed
//! time and user commands, and carries out the effects it returns.

use std::f32::consts::FRAC_PI_2;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub id: i64,
    pub path: PathBuf,
    pub duration: f32,
    /// Where the track starts when it goes on air, in seconds.
    pub start: f32,
    /// Where it is over: the auto crossfade ends here, and with nothing
    /// next the player stops here instead of playing a silent tail.
    pub end: f32,
    /// Linear gain that evens out loudness between tracks.
    pub gain: f32,
}

/// Which deck a command is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// What the speakers hear: the incoming track while a crossfade runs.
    OnAir,
    Next,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OnAir {
    pub slot: Slot,
    pub pos: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Incoming {
    pub slot: Slot,
    pub elapsed: f32,
    pub fade_len: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Open `slot` on the master output at `at` seconds, at full gain.
    StartOnAir {
        slot: Slot,
        at: f32,
    },
    /// Open `slot` from its start point as the second voice of a crossfade.
    StartIncoming {
        slot: Slot,
    },
    /// The crossfade is over: the incoming voice becomes the on-air one.
    FinishCrossfade,
    SeekOnAir {
        at: f32,
    },
    StopAll,
    StartPreview {
        slot: Slot,
    },
    StopPreview,
    /// Next has left deck B for the speakers (crossfade or play).
    NextTaken {
        id: i64,
    },
}

#[derive(Debug, Clone)]
pub struct Transport {
    pub crossfade_secs: f32,
    pub on_air: Option<OnAir>,
    pub next: Option<Slot>,
    pub incoming: Option<Incoming>,
    pub playing: bool,
    pub previewing: bool,
    /// Loudness gains are applied; off, every track plays as recorded.
    pub normalize: bool,
}

impl Transport {
    pub fn new(crossfade_secs: f32) -> Self {
        Self {
            crossfade_secs,
            on_air: None,
            next: None,
            incoming: None,
            playing: false,
            previewing: false,
            normalize: true,
        }
    }

    pub fn load_next(&mut self, slot: Slot) -> Vec<Effect> {
        self.next = Some(slot);
        self.stop_preview()
    }

    /// Empties deck B (and stops its preview).
    pub fn clear_next(&mut self) -> Vec<Effect> {
        self.next = None;
        self.stop_preview()
    }

    /// Puts a track straight on deck A, ready from its start and paused.
    /// Refused while A is playing or mixing: that would cut the speakers.
    pub fn load_on_air(&mut self, slot: Slot) -> Option<Vec<Effect>> {
        if self.incoming.is_some() || (self.playing && self.on_air.is_some()) {
            return None;
        }
        self.playing = false;
        self.on_air = Some(OnAir {
            pos: slot.start,
            slot: slot.clone(),
        });
        Some(vec![Effect::StartOnAir {
            at: slot.start,
            slot,
        }])
    }

    pub fn set_start(&mut self, secs: f32) -> Vec<Effect> {
        let Some(next) = &mut self.next else {
            return vec![];
        };
        next.start = secs.clamp(0.0, (next.duration - 0.5).max(0.0));
        next.end = next.end.max((next.start + 0.5).min(next.duration));
        if self.previewing {
            vec![Effect::StartPreview { slot: next.clone() }]
        } else {
            vec![]
        }
    }

    pub fn toggle_preview(&mut self) -> Vec<Effect> {
        match &self.next {
            Some(_) if self.previewing => self.stop_preview(),
            Some(next) => {
                self.previewing = true;
                vec![Effect::StartPreview { slot: next.clone() }]
            }
            None => vec![],
        }
    }

    /// Resumes a paused track, or with nothing on air sends Next straight out
    /// without a fade.
    pub fn play(&mut self) -> Vec<Effect> {
        if self.on_air.is_some() {
            self.playing = true;
            return vec![];
        }
        let Some(slot) = self.next.take() else {
            return vec![];
        };
        let mut fx = self.stop_preview();
        fx.push(Effect::NextTaken { id: slot.id });
        fx.push(Effect::StartOnAir {
            slot: slot.clone(),
            at: slot.start,
        });
        self.on_air = Some(OnAir {
            pos: slot.start,
            slot,
        });
        self.playing = true;
        fx
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn stop(&mut self) -> Vec<Effect> {
        self.on_air = None;
        self.incoming = None;
        self.playing = false;
        vec![Effect::StopAll]
    }

    pub fn crossfade_now(&mut self) -> Vec<Effect> {
        if self.on_air.is_none() {
            return self.play();
        }
        self.playing = true;
        self.begin_crossfade(self.crossfade_secs)
    }

    pub fn seek(&mut self, secs: f32) -> Vec<Effect> {
        match &mut self.on_air {
            Some(on) if self.incoming.is_none() => {
                on.pos = secs.clamp(0.0, on.slot.duration);
                vec![Effect::SeekOnAir { at: on.pos }]
            }
            _ => vec![],
        }
    }

    /// Moves time on by `dt` seconds of master output.
    pub fn advance(&mut self, dt: f32) -> Vec<Effect> {
        if !self.playing {
            return vec![];
        }
        let Some(on) = &mut self.on_air else {
            return vec![];
        };
        on.pos += dt;
        let remaining = on.slot.end.min(on.slot.duration) - on.pos;

        if let Some(inc) = &mut self.incoming {
            inc.elapsed += dt;
            if inc.elapsed >= inc.fade_len {
                return self.finish_crossfade();
            }
            return vec![];
        }
        if self.next.is_some() && remaining <= self.crossfade_secs {
            // Fade over whatever is left, so it ends when the track does.
            return self.begin_crossfade(remaining.max(0.05));
        }
        if remaining <= 0.0 {
            return self.stop();
        }
        vec![]
    }

    /// The on-air file ran out (its real length can differ from its tags).
    pub fn on_air_ended(&mut self) -> Vec<Effect> {
        if self.incoming.is_some() {
            return self.finish_crossfade();
        }
        let mut fx = self.stop();
        if self.next.is_some() {
            fx.extend(self.play());
        }
        fx
    }

    /// Gains of the on-air and incoming voices: equal power, so the sum
    /// sounds as loud mid-fade as either track alone.
    /// Moves where a deck's track is over. On air, the auto crossfade
    /// follows; a running crossfade is left alone.
    pub fn set_end(&mut self, which: Which, secs: f32) {
        let slot = match which {
            Which::Next => self.next.as_mut(),
            Which::OnAir => match &mut self.incoming {
                Some(_) => None,
                None => self.on_air.as_mut().map(|o| &mut o.slot),
            },
        };
        if let Some(slot) = slot {
            slot.end = secs.clamp((slot.start + 0.5).min(slot.duration), slot.duration);
        }
    }

    /// The crossfade gains of the on-air and incoming voices.
    pub fn gains(&self) -> (f32, f32) {
        match &self.incoming {
            Some(inc) => {
                let t = (inc.elapsed / inc.fade_len).clamp(0.0, 1.0) * FRAC_PI_2;
                (t.cos(), t.sin())
            }
            None => (1.0, 0.0),
        }
    }

    /// A track's own loudness gain, or unity with normalization off.
    pub fn level(&self, slot: &Slot) -> f32 {
        if self.normalize { slot.gain } else { 1.0 }
    }

    fn begin_crossfade(&mut self, fade_len: f32) -> Vec<Effect> {
        if self.incoming.is_some() {
            return vec![];
        }
        let Some(slot) = self.next.take() else {
            return vec![];
        };
        let mut fx = self.stop_preview();
        fx.push(Effect::NextTaken { id: slot.id });
        fx.push(Effect::StartIncoming { slot: slot.clone() });
        self.incoming = Some(Incoming {
            slot,
            elapsed: 0.0,
            fade_len,
        });
        fx
    }

    fn finish_crossfade(&mut self) -> Vec<Effect> {
        let Some(inc) = self.incoming.take() else {
            return vec![];
        };
        self.on_air = Some(OnAir {
            pos: inc.slot.start + inc.elapsed,
            slot: inc.slot,
        });
        vec![Effect::FinishCrossfade]
    }

    fn stop_preview(&mut self) -> Vec<Effect> {
        if std::mem::take(&mut self.previewing) {
            vec![Effect::StopPreview]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: i64, duration: f32, start: f32) -> Slot {
        Slot {
            id,
            path: format!("/m/{id}.flac").into(),
            duration,
            start,
            end: duration,
            gain: 1.0,
        }
    }

    fn run(t: &mut Transport, secs: f32) -> Vec<Effect> {
        let mut fx = Vec::new();
        for _ in 0..(secs * 100.0).round() as usize {
            fx.extend(t.advance(0.01));
        }
        fx
    }

    fn on_air_from(next: Slot) -> Transport {
        let mut t = Transport::new(5.0);
        t.load_next(next);
        t.play();
        t
    }

    #[test]
    fn play_with_nothing_on_air_starts_next_without_fade() {
        let mut t = Transport::new(5.0);
        t.load_next(slot(1, 100.0, 12.0));
        let fx = t.play();
        assert_eq!(
            fx,
            [
                Effect::NextTaken { id: 1 },
                Effect::StartOnAir {
                    slot: slot(1, 100.0, 12.0),
                    at: 12.0
                }
            ]
        );
        assert!(t.next.is_none() && t.playing && t.incoming.is_none());
    }

    #[test]
    fn auto_crossfade_ends_with_track() {
        let mut t = on_air_from(slot(1, 60.0, 0.0));
        t.load_next(slot(2, 200.0, 30.0));
        let fx = run(&mut t, 54.99);
        assert!(!fx.iter().any(|e| matches!(e, Effect::StartIncoming { .. })));
        let fx = run(&mut t, 0.02);
        assert!(fx.contains(&Effect::StartIncoming {
            slot: slot(2, 200.0, 30.0)
        }));
        let fx = run(&mut t, 5.0);
        assert!(fx.contains(&Effect::FinishCrossfade));
        let on = t.on_air.as_ref().unwrap();
        assert_eq!(on.slot.id, 2);
        assert!((on.pos - 35.0).abs() < 0.05, "pos {}", on.pos);
    }

    #[test]
    fn paused_holds_auto() {
        let mut t = on_air_from(slot(1, 60.0, 0.0));
        t.load_next(slot(2, 200.0, 0.0));
        run(&mut t, 50.0);
        t.pause();
        assert!(run(&mut t, 30.0).is_empty());
        assert!(t.incoming.is_none() && t.next.is_some());
    }

    #[test]
    fn auto_stops_preview() {
        let mut t = on_air_from(slot(1, 20.0, 0.0));
        t.load_next(slot(2, 200.0, 0.0));
        t.toggle_preview();
        let fx = run(&mut t, 16.0);
        assert!(fx.contains(&Effect::StopPreview));
        assert!(!t.previewing);
    }

    #[test]
    fn stops_when_next_empty() {
        let mut t = on_air_from(slot(1, 10.0, 0.0));
        let fx = run(&mut t, 10.5);
        assert!(fx.contains(&Effect::StopAll));
        assert!(t.on_air.is_none() && !t.playing);
    }

    #[test]
    fn crossfade_promotes_next_and_clears_slot() {
        let mut t = on_air_from(slot(1, 300.0, 0.0));
        t.load_next(slot(2, 300.0, 48.0));
        run(&mut t, 10.0);
        t.crossfade_now();
        assert!(t.next.is_none());
        run(&mut t, 2.5);
        let (a, b) = t.gains();
        assert!((a * a + b * b - 1.0).abs() < 1e-3 && (a - b).abs() < 0.05);
        let fx = run(&mut t, 2.6);
        assert!(fx.contains(&Effect::FinishCrossfade));
        assert_eq!(t.on_air.as_ref().unwrap().slot.id, 2);
        assert_eq!(t.gains(), (1.0, 0.0));
    }

    #[test]
    fn loading_replaces_next_and_stops_its_preview() {
        let mut t = Transport::new(5.0);
        t.load_next(slot(1, 100.0, 0.0));
        t.toggle_preview();
        assert_eq!(t.load_next(slot(2, 100.0, 0.0)), [Effect::StopPreview]);
        assert_eq!(t.next.as_ref().unwrap().id, 2);
    }

    #[test]
    fn load_on_air_waits_for_play_and_never_cuts_a_playing_deck() {
        let mut t = Transport::new(5.0);
        let fx = t.load_on_air(slot(1, 100.0, 0.0)).unwrap();
        assert_eq!(
            fx,
            [Effect::StartOnAir {
                slot: slot(1, 100.0, 0.0),
                at: 0.0
            }]
        );
        assert!(!t.playing);
        assert!(run(&mut t, 1.0).is_empty(), "loaded but not playing yet");
        t.play();
        assert!(t.playing && t.on_air.as_ref().unwrap().slot.id == 1);

        assert!(t.load_on_air(slot(2, 100.0, 0.0)).is_none());
        assert_eq!(t.on_air.as_ref().unwrap().slot.id, 1);

        t.pause();
        assert!(t.load_on_air(slot(2, 100.0, 0.0)).is_some());
        assert_eq!(t.on_air.as_ref().unwrap().slot.id, 2);
    }

    #[test]
    fn early_end_of_file_still_hands_over() {
        let mut t = on_air_from(slot(1, 100.0, 0.0));
        t.load_next(slot(2, 100.0, 7.0));
        let fx = t.on_air_ended();
        assert!(fx.contains(&Effect::StartOnAir {
            slot: slot(2, 100.0, 7.0),
            at: 7.0
        }));
        assert!(t.playing);
    }

    #[test]
    fn auto_crossfade_ends_on_end_point_not_end_of_file() {
        let mut t = on_air_from(Slot {
            end: 50.0,
            ..slot(1, 60.0, 0.0)
        });
        t.load_next(slot(2, 200.0, 0.0));
        assert!(run(&mut t, 44.9).is_empty());
        let fx = run(&mut t, 0.2);
        assert!(fx.contains(&Effect::NextTaken { id: 2 }));
        let fx = run(&mut t, 5.0);
        assert!(fx.contains(&Effect::FinishCrossfade));
    }

    #[test]
    fn stops_at_end_point_when_next_empty() {
        let mut t = on_air_from(Slot {
            end: 8.0,
            ..slot(1, 10.0, 0.0)
        });
        assert!(!run(&mut t, 7.9).contains(&Effect::StopAll));
        assert!(run(&mut t, 0.2).contains(&Effect::StopAll));
    }

    #[test]
    fn set_end_moves_the_auto_crossfade_and_stays_after_start() {
        let mut t = on_air_from(slot(1, 100.0, 10.0));
        t.load_next(slot(2, 100.0, 0.0));
        t.set_end(Which::OnAir, 40.0);
        assert!(run(&mut t, 24.0).is_empty());
        assert!(run(&mut t, 1.1).contains(&Effect::NextTaken { id: 2 }));

        t.set_end(Which::Next, -3.0);
        let mut t = Transport::new(5.0);
        t.load_next(slot(3, 100.0, 20.0));
        t.set_end(Which::Next, 5.0);
        assert_eq!(t.next.as_ref().unwrap().end, 20.5);
        t.set_start(90.0);
        t.set_end(Which::Next, 500.0);
        assert_eq!(t.next.as_ref().unwrap().end, 100.0);
    }

    #[test]
    fn crossfade_reports_next_taken() {
        let mut t = on_air_from(slot(1, 300.0, 0.0));
        t.load_next(slot(7, 300.0, 0.0));
        assert!(t.crossfade_now().contains(&Effect::NextTaken { id: 7 }));
    }

    #[test]
    fn level_follows_normalize() {
        let mut t = Transport::new(5.0);
        let s = Slot {
            gain: 0.5,
            ..slot(1, 10.0, 0.0)
        };
        assert_eq!(t.level(&s), 0.5);
        t.normalize = false;
        assert_eq!(t.level(&s), 1.0);
    }
}
