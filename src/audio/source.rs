use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use rubato::{FftFixedIn, Resampler};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

const RESAMPLE_CHUNK: usize = 1024;

/// A file decoded to interleaved stereo f32 at the rate the output device
/// runs at, starting from a chosen point.
pub struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    in_rate: u32,
    duration_secs: f32,
    /// Decoded frames still to drop after an accurate seek.
    skip_frames: u64,
    resampler: Option<ResampleState>,
    /// Interleaved stereo at the output rate, ready to hand out.
    ready: VecDeque<f32>,
    eof: bool,
}

struct ResampleState {
    inner: FftFixedIn<f32>,
    pending: [Vec<f32>; 2],
    /// Frames of filter delay still to drop from the output.
    delay: usize,
    ratio: f64,
    frames_in: u64,
    frames_out: u64,
}

impl Source {
    pub fn open(path: &Path, out_rate: u32, start_secs: f32) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
            .with_context(|| format!("unsupported file {}", path.display()))?;
        let mut format = probed.format;
        let track = format.default_track().ok_or_else(|| anyhow!("no audio track"))?;
        let track_id = track.id;
        let params = track.codec_params.clone();
        let in_rate = params.sample_rate.ok_or_else(|| anyhow!("unknown sample rate"))?;
        let duration_secs = match (params.n_frames, params.time_base) {
            (Some(n), Some(tb)) => {
                let t = tb.calc_time(n);
                t.seconds as f32 + t.frac as f32
            }
            (Some(n), None) => n as f32 / in_rate as f32,
            _ => 0.0,
        };
        let decoder = symphonia::default::get_codecs().make(&params, &DecoderOptions::default())?;

        let mut skip_frames = 0;
        if start_secs > 0.0 {
            let time = Time::new(start_secs.trunc() as u64, f64::from(start_secs.fract()));
            if let Ok(seeked) = format.seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(track_id) }) {
                skip_frames = seeked.required_ts.saturating_sub(seeked.actual_ts);
            }
        }

        let resampler = (in_rate != out_rate)
            .then(|| -> Result<ResampleState> {
                let inner = FftFixedIn::new(in_rate as usize, out_rate as usize, RESAMPLE_CHUNK, 2, 2)?;
                let delay = inner.output_delay();
                Ok(ResampleState { inner, pending: [Vec::new(), Vec::new()], delay, ratio: f64::from(out_rate) / f64::from(in_rate), frames_in: 0, frames_out: 0 })
            })
            .transpose()?;

        Ok(Self {
            format,
            decoder,
            track_id,
            in_rate,
            duration_secs,
            skip_frames,
            resampler,
            ready: VecDeque::new(),
            eof: false,
        })
    }

    pub fn duration_secs(&self) -> f32 {
        self.duration_secs
    }

    pub fn in_rate(&self) -> u32 {
        self.in_rate
    }

    /// Fills `out` (interleaved stereo) and returns the frames written; fewer
    /// than asked only at the end of the file.
    pub fn read(&mut self, out: &mut [f32]) -> usize {
        while self.ready.len() < out.len() && !self.eof {
            if !self.decode_more() {
                self.eof = true;
                self.flush_resampler();
            }
        }
        let n = out.len().min(self.ready.len()) & !1;
        for (o, s) in out.iter_mut().zip(self.ready.drain(..n)) {
            *o = s;
        }
        n / 2
    }

    /// Decodes one packet into `ready`. False at the end of the stream.
    fn decode_more(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(_) => return false,
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                // A corrupt frame is skipped, not the end of the song.
                Err(SymError::DecodeError(_)) => continue,
                Err(_) => return false,
            };
            let spec = *decoded.spec();
            let channels = spec.channels.count().max(1);
            let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            buf.copy_interleaved_ref(decoded);
            let mut frames = buf.samples().chunks_exact(channels);
            let skip = self.skip_frames.min(frames.len() as u64);
            self.skip_frames -= skip;
            let frames = frames.by_ref().skip(skip as usize);
            let stereo = frames.map(|f| if channels == 1 { [f[0], f[0]] } else { [f[0], f[1]] });
            match &mut self.resampler {
                None => {
                    for [l, r] in stereo {
                        self.ready.push_back(l);
                        self.ready.push_back(r);
                    }
                }
                Some(rs) => {
                    for [l, r] in stereo {
                        rs.pending[0].push(l);
                        rs.pending[1].push(r);
                        rs.frames_in += 1;
                    }
                    while rs.pending[0].len() >= rs.inner.input_frames_next() {
                        let need = rs.inner.input_frames_next();
                        let chunk: Vec<Vec<f32>> = rs.pending.iter_mut().map(|ch| ch.drain(..need).collect()).collect();
                        if let Ok(out) = rs.inner.process(&chunk, None) {
                            rs.push(&mut self.ready, &out);
                        }
                    }
                }
            }
            return true;
        }
    }

    fn flush_resampler(&mut self) {
        let Some(rs) = &mut self.resampler else { return };
        let rest: Vec<Vec<f32>> = rs.pending.iter_mut().map(std::mem::take).collect();
        if let Ok(out) = rs.inner.process_partial(Some(&rest), None) {
            rs.push(&mut self.ready, &out);
        }
        // One more empty pass drains the filter's tail.
        if let Ok(out) = rs.inner.process_partial::<Vec<f32>>(None, None) {
            rs.push(&mut self.ready, &out);
        }
    }
}

impl ResampleState {
    /// Queues resampled frames, minus the filter's start-up delay and minus
    /// the zero padding the last chunks carry past the real end.
    fn push(&mut self, ready: &mut VecDeque<f32>, out: &[Vec<f32>]) {
        let frames = out[0].len();
        let skip = self.delay.min(frames);
        self.delay -= skip;
        let limit = (self.frames_in as f64 * self.ratio).round() as u64;
        for i in skip..frames {
            if self.frames_out >= limit {
                break;
            }
            ready.push_back(out[0][i]);
            ready.push_back(out[1][i]);
            self.frames_out += 1;
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A 16-bit PCM stereo WAV of a 440 Hz sine.
    pub fn write_wav(path: &Path, rate: u32, secs: f32) {
        let frames = (rate as f32 * secs) as u32;
        let data_len = frames * 4;
        let mut b = Vec::with_capacity(44 + data_len as usize);
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 4).to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let v = ((i as f32 / rate as f32 * 440.0 * std::f32::consts::TAU).sin() * 16000.0) as i16;
            b.extend_from_slice(&v.to_le_bytes());
            b.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(path, b).unwrap();
    }

    fn read_all(src: &mut Source) -> usize {
        let mut buf = vec![0.0; 2048];
        let mut total = 0;
        loop {
            let n = src.read(&mut buf);
            total += n;
            if n < 1024 {
                return total;
            }
        }
    }

    #[test]
    fn reads_all_frames_at_native_rate() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        write_wav(&p, 44_100, 1.0);
        let mut src = Source::open(&p, 44_100, 0.0).unwrap();
        assert!((src.duration_secs() - 1.0).abs() < 0.01);
        assert_eq!(read_all(&mut src), 44_100);
    }

    #[test]
    fn resamples_to_48k() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        write_wav(&p, 44_100, 1.0);
        let mut src = Source::open(&p, 48_000, 0.0).unwrap();
        let n = read_all(&mut src) as f32;
        assert!((n - 48_000.0).abs() / 48_000.0 < 0.01, "got {n} frames");
    }

    #[test]
    fn start_offset_skips() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        write_wav(&p, 44_100, 2.0);
        let mut src = Source::open(&p, 44_100, 1.5).unwrap();
        let n = read_all(&mut src) as f32;
        assert!((n - 22_050.0).abs() < 200.0, "got {n} frames");
    }
}
