use std::path::Path;

use anyhow::{Context, Result};
use crossbeam_channel::Receiver;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

use crate::resample::TARGET_SR;

pub fn run(path: &Path, rx: Receiver<Vec<f32>>) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_SR,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).context("create wav writer")?;

    while let Ok(samples) = rx.recv() {
        for sample in samples {
            writer.write_sample(to_i16(sample))?;
        }
    }

    writer.finalize().context("finalize wav")?;
    Ok(())
}

/// Merge two mono 16-bit WAV files into a stereo WAV (mic=L, sys=R).
/// Pads the shorter file with silence so both channels have equal length.
pub fn merge(mic: &Path, sys: &Path, out: &Path) -> Result<()> {
    let mut mic_reader = WavReader::open(mic).context("open mic.wav for merge")?;
    let mut sys_reader = WavReader::open(sys).context("open system.wav for merge")?;

    let spec = WavSpec {
        channels: 2,
        sample_rate: TARGET_SR,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut merged = WavWriter::create(out, spec).context("create merged.wav")?;

    let mut mic_samples = mic_reader.samples::<i16>();
    let mut sys_samples = sys_reader.samples::<i16>();

    loop {
        let left = mic_samples.next().transpose().context("read mic sample")?;
        let right = sys_samples.next().transpose().context("read sys sample")?;
        match (left, right) {
            (None, None) => break,
            (left, right) => {
                merged.write_sample(left.unwrap_or(0))?;
                merged.write_sample(right.unwrap_or(0))?;
            }
        }
    }

    merged.finalize().context("finalize merged.wav")?;
    Ok(())
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;

    fn write_mono_wav(path: &Path, samples: &[i16], sr: u32) {
        let spec = WavSpec {
            channels: 1,
            sample_rate: sr,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn to_i16_clamps() {
        assert_eq!(to_i16(0.0), 0);
        assert_eq!(to_i16(1.0), i16::MAX);
        assert_eq!(to_i16(2.0), i16::MAX);
        assert_eq!(to_i16(-2.0), -i16::MAX);
    }

    #[test]
    fn merge_interleaves_channels_and_pads() {
        let tmp = tempfile::tempdir().unwrap();
        let mic = tmp.path().join("mic.wav");
        let sys = tmp.path().join("sys.wav");
        let out = tmp.path().join("merged.wav");

        write_mono_wav(&mic, &[100, 200, 300, 400], TARGET_SR);
        write_mono_wav(&sys, &[-100, -200], TARGET_SR); // shorter — should pad with 0

        merge(&mic, &sys, &out).unwrap();

        let reader = WavReader::open(&out).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, TARGET_SR);
        let samples: Vec<i16> = reader
            .into_samples::<i16>()
            .map(|sample| sample.unwrap())
            .collect();
        // Stereo interleaved: L, R, L, R, ...
        assert_eq!(samples, vec![100, -100, 200, -200, 300, 0, 400, 0]);
    }
}
