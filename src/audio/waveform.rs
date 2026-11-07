use std::path::Path;

use anyhow::Result;

use super::source::Source;

/// Peak level (0..=1) of each of `buckets` equal slices of the file.
pub fn peaks(path: &Path, buckets: usize) -> Result<Vec<f32>> {
    // Decode at the file's own rate: resampling would only cost time here.
    let probe = Source::open(path, 44_100, 0.0)?;
    let mut src = Source::open(path, probe.in_rate(), 0.0)?;
    drop(probe);

    const BLOCK: usize = 512;
    let mut blocks = Vec::new();
    let mut buf = vec![0.0f32; BLOCK * 2];
    loop {
        let n = src.read(&mut buf);
        if n == 0 {
            break;
        }
        blocks.push(buf[..n * 2].iter().fold(0.0f32, |m, s| m.max(s.abs())));
        if n < BLOCK {
            break;
        }
    }
    if blocks.is_empty() {
        return Ok(vec![0.0; buckets]);
    }
    Ok((0..buckets)
        .map(|b| {
            let from = b * blocks.len() / buckets;
            let to = ((b + 1) * blocks.len() / buckets).max(from + 1).min(blocks.len());
            blocks[from.min(blocks.len() - 1)..to].iter().fold(0.0f32, |m, &v| m.max(v)).min(1.0)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_len_and_range() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        crate::audio::source::tests::write_wav(&p, 44_100, 1.0);
        let peaks = peaks(&p, 200).unwrap();
        assert_eq!(peaks.len(), 200);
        // The fixture is a sine at 16000/32768 of full scale.
        assert!(peaks.iter().all(|&v| (0.45..=0.5).contains(&v)), "{:?}", &peaks[..5]);
    }
}
