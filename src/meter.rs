use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Lock-free level meter shared between the resample worker (writer) and the
/// display thread (reader). Stores a 0..=1 level as f32 bits in an AtomicU32 so
/// neither side ever blocks — safe to read from display while the worker writes.
#[derive(Clone, Default)]
pub struct Meter(Arc<AtomicU32>);

impl Meter {
    pub fn new() -> Self {
        Meter(Arc::new(AtomicU32::new(0)))
    }

    /// Current level, 0.0..=1.0.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, level: f32) {
        self.0.store(level.to_bits(), Ordering::Relaxed);
    }

    /// Fold a chunk of samples into the meter: fast attack (snap up to the
    /// chunk's peak), slow release (fall by `decay` when the chunk is quieter).
    /// Non-finite samples are ignored; the stored level is clamped to 0..=1.
    pub fn update_from(&self, samples: &[f32], decay: f32) {
        let peak = samples.iter().fold(0.0_f32, |highest, &sample| {
            let magnitude = sample.abs();
            if magnitude.is_finite() && magnitude > highest {
                magnitude
            } else {
                highest
            }
        });
        let prev = self.level();
        let next = if peak >= prev { peak } else { prev * decay };
        self.set(next.clamp(0.0, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_reads_zero() {
        let meter = Meter::new();
        meter.update_from(&[0.0; 128], 0.9);
        assert!(
            meter.level() < 1e-6,
            "silence should read ~0, got {}",
            meter.level()
        );
    }

    #[test]
    fn full_scale_reads_one() {
        let meter = Meter::new();
        meter.update_from(&[1.0, -1.0, 0.5], 0.9);
        assert!(
            (meter.level() - 1.0).abs() < 1e-6,
            "full scale should read ~1, got {}",
            meter.level()
        );
    }

    #[test]
    fn attack_instant_release_decays() {
        let meter = Meter::new();
        meter.update_from(&[0.8], 0.5);
        assert!((meter.level() - 0.8).abs() < 1e-6); // instant attack
        meter.update_from(&[0.0], 0.5);
        assert!((meter.level() - 0.4).abs() < 1e-6); // falls by decay
        meter.update_from(&[0.0], 0.5);
        assert!((meter.level() - 0.2).abs() < 1e-6);
        for _ in 0..20 {
            meter.update_from(&[0.0], 0.5);
        }
        assert!(
            meter.level() < 0.01,
            "should keep decaying toward 0, got {}",
            meter.level()
        );
    }

    #[test]
    fn clamps_and_ignores_nonfinite() {
        let meter = Meter::new();
        meter.update_from(&[f32::NAN, f32::INFINITY, 2.5, 0.3], 0.9);
        assert!(
            (meter.level() - 1.0).abs() < 1e-6,
            "2.5 clamps to 1, inf/nan ignored, got {}",
            meter.level()
        );
    }
}
