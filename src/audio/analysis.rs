//! One decode of a file gives everything the decks need to know about it:
//! the waveform, where the sound really starts and ends, and how loud it is.

use std::path::Path;

use anyhow::Result;
use ebur128::{EbuR128, Mode};

use super::source::Source;

/// Below this a block counts as silence: Mixxx's "Skip Silence" threshold.
const SILENCE_DBFS: f32 = -60.0;
/// Every track is brought to this loudness, the streaming services' level.
pub const TARGET_LUFS: f32 = -14.0;
const MAX_GAIN_DB: f32 = 12.0;
const BLOCK: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    /// Peak level (0..=1) of each of the requested equal slices.
    pub peaks: Vec<f32>,
    /// First and last instant above the silence threshold, in seconds.
    pub sound_start: f32,
    pub sound_end: f32,
    /// EBU R128 integrated loudness; None for a file that is all silence.
    pub loudness: Option<f32>,
    /// Highest sample, 0..=1 (or above, for a clipped file).
    pub peak: f32,
    /// Counted on the decoded frames, so it beats the tags'.
    pub duration: f32,
}

impl Analysis {
    /// The linear gain that brings the track to `TARGET_LUFS` without
    /// pushing its peak past full scale.
    pub fn gain(&self) -> f32 {
        let Some(lufs) = self.loudness else {
            return 1.0;
        };
        let wanted = (TARGET_LUFS - lufs).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
        let headroom = if self.peak > 0.0 {
            -20.0 * self.peak.log10()
        } else {
            MAX_GAIN_DB
        };
        db_to_gain(wanted.min(headroom))
    }
}

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn analyse(path: &Path, buckets: usize) -> Result<Analysis> {
    // Decode at the file's own rate: resampling would only cost time here.
    let probe = Source::open(path, 44_100, 0.0)?;
    let rate = probe.in_rate();
    drop(probe);
    let mut src = Source::open(path, rate, 0.0)?;
    let mut meter = EbuR128::new(2, rate, Mode::I | Mode::SAMPLE_PEAK)?;

    let threshold = 10f32.powf(SILENCE_DBFS / 20.0);
    let mut blocks = Vec::new();
    let mut frames = 0usize;
    let (mut first, mut last) = (None, 0usize);
    let mut buf = vec![0.0f32; BLOCK * 2];
    loop {
        let n = src.read(&mut buf);
        if n == 0 {
            break;
        }
        let samples = &buf[..n * 2];
        meter.add_frames_f32(samples)?;
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if peak > threshold {
            // Find the exact frames inside the block, not just the block.
            let above = |f: &[f32; 2]| f[0].abs() > threshold || f[1].abs() > threshold;
            let (pairs, _) = samples.as_chunks::<2>();
            if first.is_none() {
                first = pairs.iter().position(above).map(|i| frames + i);
            }
            if let Some(i) = pairs.iter().rposition(above) {
                last = frames + i + 1;
            }
        }
        blocks.push(peak);
        frames += n;
        if n < BLOCK {
            break;
        }
    }

    let secs = |f: usize| f as f32 / rate as f32;
    let duration = secs(frames);
    let loudness = meter
        .loudness_global()
        .ok()
        .filter(|l| l.is_finite())
        .map(|l| l as f32);
    let peak = (0..2)
        .filter_map(|ch| meter.sample_peak(ch).ok())
        .fold(0.0f64, f64::max) as f32;
    let (sound_start, sound_end) = match first {
        Some(f) => (secs(f), secs(last)),
        None => (0.0, duration),
    };
    Ok(Analysis {
        peaks: bucket_peaks(&blocks, buckets),
        sound_start,
        sound_end,
        loudness,
        peak,
        duration,
    })
}

fn bucket_peaks(blocks: &[f32], buckets: usize) -> Vec<f32> {
    if blocks.is_empty() {
        return vec![0.0; buckets];
    }
    (0..buckets)
        .map(|b| {
            let from = (b * blocks.len() / buckets).min(blocks.len() - 1);
            let to = ((b + 1) * blocks.len() / buckets)
                .max(from + 1)
                .min(blocks.len());
            blocks[from..to]
                .iter()
                .fold(0.0f32, |m, &v| m.max(v))
                .min(1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::source::tests::{write_wav, write_wav_samples};

    #[test]
    fn peaks_len_and_range() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        write_wav(&p, 44_100, 1.0);
        let a = analyse(&p, 200).unwrap();
        assert_eq!(a.peaks.len(), 200);
        // The fixture is a sine at 16000/32768 of full scale.
        assert!(a.peaks.iter().all(|&v| (0.45..=0.5).contains(&v)));
        assert!((a.duration - 1.0).abs() < 0.01);
    }

    #[test]
    fn finds_sound_between_silences() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        let rate = 8_000;
        // 1 s silence, 2 s sine, 3 s of hiss below -60 dBFS.
        let mut s = vec![0i16; rate];
        s.extend((0..2 * rate).map(|i| ((i as f32 * 0.3).sin() * 12000.0) as i16));
        s.extend((0..3 * rate).map(|i| if i % 2 == 0 { 20 } else { -20 }));
        write_wav_samples(&p, rate as u32, &s);
        let a = analyse(&p, 10).unwrap();
        assert!((a.sound_start - 1.0).abs() < 0.01, "{}", a.sound_start);
        assert!((a.sound_end - 3.0).abs() < 0.01, "{}", a.sound_end);
        assert!((a.duration - 6.0).abs() < 0.01);
    }

    #[test]
    fn silent_file_keeps_whole_length_and_unity_gain() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        write_wav_samples(&p, 8_000, &[0; 16_000]);
        let a = analyse(&p, 10).unwrap();
        assert_eq!((a.sound_start, a.sound_end), (0.0, 2.0));
        assert_eq!(a.gain(), 1.0);
    }

    #[test]
    fn gain_reaches_target_but_never_clips() {
        let quiet = Analysis {
            peaks: vec![],
            sound_start: 0.0,
            sound_end: 1.0,
            loudness: Some(-20.0),
            peak: 0.1,
            duration: 1.0,
        };
        // 6 dB up to reach -14 LUFS: room enough under a 0.1 peak.
        assert!((quiet.gain() - db_to_gain(6.0)).abs() < 1e-4);

        let peaky = Analysis {
            peak: 0.9,
            ..quiet.clone()
        };
        // Only as far as the peak allows.
        assert!((peaky.gain() * 0.9 - 1.0).abs() < 1e-4);

        let loud = Analysis {
            loudness: Some(-8.0),
            peak: 1.0,
            ..quiet
        };
        assert!((loud.gain() - db_to_gain(-6.0)).abs() < 1e-4);
    }
}
