//! Loads a file into deck B and previews it on a cue device:
//! `cargo run --example cue_smoke -- <file> <cue device>`
#![allow(dead_code)]

#[path = "../src/audio/mod.rs"]
mod audio;

use std::time::{Duration, Instant};

use audio::engine::{Command, Engine, EngineEvent, Role};
use audio::transport::Slot;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("file");
    let cue = args.next().expect("cue device");
    let (engine, events) = Engine::start(3.0);
    engine.send(Command::SetDevices {
        master: audio::output::default_device(),
        cue: Some(cue),
    });
    engine.send(Command::SetVolume(Role::Cue, 0.4));
    engine.send(Command::LoadNext(Slot {
        id: 1,
        path: path.into(),
        duration: 0.0,
        start: 0.0,
    }));
    engine.send(Command::TogglePreview);
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        if let Ok(e) = events.recv_timeout(Duration::from_millis(200)) {
            match e {
                EngineEvent::State(s) => println!(
                    "state next={:?} previewing={}",
                    s.next.map(|n| n.id),
                    s.previewing
                ),
                other => println!("event {other:?}"),
            }
        }
    }
    engine.send(Command::TogglePreview);
}
