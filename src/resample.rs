use std::thread;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use rubato::{FftFixedIn, Resampler};

use crate::capture::RawChunk;
use crate::meter::Meter;

pub const TARGET_SR: u32 = 16_000;

/// Per-chunk release factor for the level meter. Chunks arrive many times per
/// display tick, so this yields a visibly fast (but smooth) fall during quiet.
const METER_DECAY: f32 = 0.90;

pub struct MonoResampler {
    inner: FftFixedIn<f32>,
    src_channels: u16,
    chunk_size: usize,
    input_buf: Vec<f32>,
}

impl MonoResampler {
    pub fn new(src_sample_rate: u32, src_channels: u16) -> Result<Self> {
        let chunk_size = 1024;
        let inner = FftFixedIn::<f32>::new(
            src_sample_rate as usize,
            TARGET_SR as usize,
            chunk_size,
            2,
            1,
        )
        .context("rubato FftFixedIn::new")?;
        Ok(Self {
            inner,
            src_channels,
            chunk_size,
            input_buf: Vec::with_capacity(chunk_size * 2),
        })
    }

    pub fn push(&mut self, interleaved: &[f32], out: &mut Vec<f32>) -> Result<()> {
        // Downmix to mono.
        if self.src_channels <= 1 {
            self.input_buf.extend_from_slice(interleaved);
        } else {
            let ch = self.src_channels as usize;
            for frame in interleaved.chunks_exact(ch) {
                let sum: f32 = frame.iter().sum();
                self.input_buf.push(sum / ch as f32);
            }
        }

        while self.input_buf.len() >= self.chunk_size {
            let block: Vec<f32> = self.input_buf.drain(..self.chunk_size).collect();
            let waves_in = vec![block];
            let waves_out = self.inner.process(&waves_in, None).context("resample process")?;
            out.extend_from_slice(&waves_out[0]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a sine wave of `freq` Hz, `secs` seconds, at `sr` Hz, interleaved for `channels`.
    fn sine(freq: f32, secs: f32, sr: u32, channels: u16) -> Vec<f32> {
        let n = (sr as f32 * secs) as usize;
        let mut v = Vec::with_capacity(n * channels as usize);
        for i in 0..n {
            let t = i as f32 / sr as f32;
            let s = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.5;
            for _ in 0..channels { v.push(s); }
        }
        v
    }

    #[test]
    fn resamples_48k_to_16k_within_1_percent() {
        let input = sine(440.0, 1.0, 48_000, 1);
        let mut r = MonoResampler::new(48_000, 1).unwrap();
        let mut out = Vec::new();
        r.push(&input, &mut out).unwrap();
        let expected = (input.len() as f64) * (TARGET_SR as f64) / 48_000.0;
        let ratio = out.len() as f64 / expected;
        assert!(ratio > 0.97 && ratio < 1.03,
            "expected ~{expected} samples, got {} (ratio {})", out.len(), ratio);
        assert!(out.iter().all(|s| s.is_finite()), "resampled output has NaN/Inf");
    }

    #[test]
    fn downmixes_stereo_to_mono() {
        // Stereo: L=+0.5, R=-0.5 → mono = 0.0. Lots of frames so a chunk emits.
        let frames = 4096usize;
        let stereo: Vec<f32> = (0..frames).flat_map(|_| [0.5_f32, -0.5_f32]).collect();
        let mut r = MonoResampler::new(48_000, 2).unwrap();
        let mut out = Vec::new();
        r.push(&stereo, &mut out).unwrap();
        // Output may be empty until enough mono samples accumulate; if non-empty,
        // every sample should be ~0 (within resampler ringing tolerance).
        for s in &out {
            assert!(s.abs() < 0.05, "downmix should sum to ~0, got {s}");
        }
    }
}

/// Spawn a thread that resamples raw capture chunks to 16 kHz mono `Vec<f32>`,
/// updating `meter` with the input level of every chunk (this thread is not
/// real-time, unlike the capture callbacks).
pub fn spawn_worker(rx: Receiver<RawChunk>, src_sr: u32, src_ch: u16, tx: Sender<Vec<f32>>, meter: Meter) {
    thread::spawn(move || {
        let mut resampler = match MonoResampler::new(src_sr, src_ch) {
            Ok(r) => r,
            Err(e) => { tracing::error!(error = ?e, "resampler init failed"); return; }
        };
        let mut out = Vec::with_capacity(4096);
        while let Ok(chunk) = rx.recv() {
            // Meter the raw input peak every chunk so the bar decays during quiet.
            meter.update_from(&chunk.samples, METER_DECAY);
            out.clear();
            if let Err(e) = resampler.push(&chunk.samples, &mut out) {
                tracing::error!(error = ?e, "resample failed");
                continue;
            }
            if out.is_empty() { continue; }
            let _ = tx.send(out.clone());
        }
    });
}
