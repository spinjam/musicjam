//! Plays a file on the default output for a few seconds:
//! `cargo run --example engine_smoke -- <file>`
#![allow(dead_code)]

#[path = "../src/audio/mod.rs"]
mod audio;

use std::time::Duration;

use audio::engine::{Command, Engine, EngineEvent};
use audio::transport::Slot;

fn main() {
    let path = std::env::args().nth(1).expect("usage: engine_smoke <file>");
    let device = audio::output::default_device().expect("no output device");
    println!("devices: {:?}\nusing: {device}", audio::output::list_devices());
    let (engine, events) = Engine::start(3.0);
    engine.send(Command::SetDevices { master: Some(device), cue: None });
    engine.send(Command::LoadNext(Slot { id: 1, path: path.into(), duration: 0.0, start: 0.0 }));
    engine.send(Command::Play);
    let until = std::time::Instant::now() + Duration::from_secs(4);
    let mut last = None;
    while std::time::Instant::now() < until {
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(EngineEvent::State(s)) => last = Some(s),
            Ok(other) => println!("event: {other:?}"),
            Err(_) => {}
        }
    }
    println!("last state: {last:?}");
}
