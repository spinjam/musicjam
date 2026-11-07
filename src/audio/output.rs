use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// How much audio sits between the render thread and the device. Enough to
/// ride over opening a file mid-crossfade; latency does not matter here since
/// nothing is played live against it.
const RING_SECS: f32 = 0.25;

pub fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut names: Vec<String> = host
        .output_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    names.sort();
    names.dedup();
    names
}

pub fn default_device() -> Option<String> {
    cpal::default_host().default_output_device().and_then(|d| d.name().ok())
}

/// An open output device. The render thread pushes interleaved stereo into
/// `producer`; the device callback drains it, padding with silence if the
/// renderer falls behind.
pub struct Output {
    _stream: cpal::Stream,
    pub producer: rtrb::Producer<f32>,
    pub rate: u32,
    lost: Arc<AtomicBool>,
}

impl Output {
    pub fn open(name: &str) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .output_devices()?
            .find(|d| d.name().is_ok_and(|n| n == name))
            .ok_or_else(|| anyhow!("device not found: {name}"))?;
        let supported = device.default_output_config()?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return Err(anyhow!("{name}: unsupported sample format {:?}", supported.sample_format()));
        }
        let config: cpal::StreamConfig = supported.into();
        let rate = config.sample_rate.0;
        let channels = config.channels as usize;

        let (producer, mut consumer) = rtrb::RingBuffer::<f32>::new((rate as f32 * RING_SECS) as usize * 2);
        let lost = Arc::new(AtomicBool::new(false));
        let lost_cb = lost.clone();
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                for frame in data.chunks_mut(channels) {
                    let (l, r) = match (consumer.pop(), consumer.pop()) {
                        (Ok(l), Ok(r)) => (l, r),
                        _ => (0.0, 0.0),
                    };
                    for (i, s) in frame.iter_mut().enumerate() {
                        *s = match i {
                            0 => l,
                            1 => r,
                            _ => 0.0,
                        };
                    }
                    if channels == 1 {
                        frame[0] = 0.5 * (l + r);
                    }
                }
            },
            move |_err| lost_cb.store(true, Ordering::Relaxed),
            None,
        )?;
        stream.play()?;
        Ok(Self { _stream: stream, producer, rate, lost })
    }

    /// True once the device has gone away (unplugged, out of Bluetooth range).
    pub fn is_lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }

    /// Frames the ring can take right now.
    pub fn free_frames(&self) -> usize {
        self.producer.slots() / 2
    }

    pub fn push(&mut self, interleaved: &[f32]) {
        for &s in interleaved {
            if self.producer.push(s).is_err() {
                break;
            }
        }
    }
}

/// A short tone, first on the left then on the right, so the listener can
/// tell which device this is and that its channels are the right way round.
pub struct TestTone {
    rate: f32,
    frame: usize,
}

impl TestTone {
    const SIDE_SECS: f32 = 0.4;

    pub fn new(rate: u32) -> Self {
        Self { rate: rate as f32, frame: 0 }
    }

    /// Adds the tone into `out` (interleaved stereo). False once it is over.
    pub fn mix_into(&mut self, out: &mut [f32]) -> bool {
        let side_frames = (self.rate * Self::SIDE_SECS) as usize;
        for f in out.as_chunks_mut::<2>().0 {
            if self.frame >= side_frames * 2 {
                return false;
            }
            let t = self.frame as f32 / self.rate;
            let v = (t * 440.0 * std::f32::consts::TAU).sin() * 0.25;
            f[usize::from(self.frame >= side_frames)] += v;
            self.frame += 1;
        }
        self.frame < side_frames * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tone_left_then_right() {
        let mut tone = TestTone::new(1000);
        let mut buf = vec![0.0; 2 * 800];
        assert!(!tone.mix_into(&mut buf));
        let left: f32 = buf[..800].chunks(2).map(|f| f[1].abs()).sum();
        let right: f32 = buf[800..].chunks(2).map(|f| f[0].abs()).sum();
        assert_eq!((left, right), (0.0, 0.0));
        assert!(buf[..800].chunks(2).any(|f| f[0] != 0.0));
    }
}
