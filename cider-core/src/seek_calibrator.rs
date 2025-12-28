//! Seek offset calibration for listener sync
//!
//! Cider's seek operation has inherent latency due to buffering.
//! This module adaptively calibrates the seek offset based on observed drift
//! to minimize sync error between host and listeners.

use std::sync::{Arc, RwLock};

/// Default seek offset when no calibration has occurred (ms)
const DEFAULT_SEEK_OFFSET_MS: u64 = 500;

/// Minimum seek offset (ms) - don't go below this
const MIN_SEEK_OFFSET_MS: u64 = 100;

/// Maximum seek offset (ms) - don't go above this
const MAX_SEEK_OFFSET_MS: u64 = 2000;

/// EMA smoothing factor (0.0-1.0)
/// Higher = more responsive to recent measurements, more variance
/// Lower = more stable, slower to adapt
const EMA_ALPHA: f64 = 0.15;

/// Maximum drift for full-weight calibration (ms)
/// Measurements beyond this get reduced weight
const MAX_CALIBRATION_DRIFT_MS: i64 = 1500;

/// EMA alpha for outlier measurements (very slow learning)
/// We still learn from outliers, just much more slowly
const OUTLIER_ALPHA: f64 = 0.05;

/// A recorded calibration sample
#[derive(Debug, Clone)]
pub struct CalibrationSample {
    /// Drift measured after seek (positive = ahead, negative = behind)
    pub drift_ms: i64,
    /// The ideal offset this sample suggested
    pub ideal_offset_ms: i64,
    /// The offset after applying this sample
    pub new_offset_ms: u64,
    /// Whether this sample was rejected as outlier
    pub rejected: bool,
}

/// Maximum number of samples to keep in history
const MAX_SAMPLE_HISTORY: usize = 10;

/// Calibrates seek offset based on observed drift
#[derive(Debug)]
pub struct SeekCalibrator {
    /// Current calibrated seek offset in milliseconds
    offset_ms: f64,
    /// Number of samples received (for initial calibration)
    sample_count: u32,
    /// Whether we're waiting to measure the result of a seek operation
    awaiting_measurement: bool,
    /// Recent sample history for debug display
    sample_history: Vec<CalibrationSample>,
}

impl SeekCalibrator {
    pub fn new() -> Self {
        Self {
            offset_ms: DEFAULT_SEEK_OFFSET_MS as f64,
            sample_count: 0,
            awaiting_measurement: false,
            sample_history: Vec::new(),
        }
    }

    /// Get the current calibrated seek offset in milliseconds
    pub fn offset_ms(&self) -> u64 {
        self.offset_ms.round() as u64
    }

    /// Check if we're waiting to measure after a seek
    pub fn is_awaiting_measurement(&self) -> bool {
        self.awaiting_measurement
    }

    /// Preview what ideal offset would result from a given drift measurement.
    /// Returns None if the drift would be rejected as an outlier.
    pub fn preview_calibration(&self, drift_ms: i64) -> Option<i64> {
        if drift_ms.abs() > MAX_CALIBRATION_DRIFT_MS {
            return None; // Would be rejected as outlier
        }
        // ideal_offset = current_offset - drift
        let ideal = self.offset_ms - drift_ms as f64;
        Some(ideal.round() as i64)
    }

    /// Mark that a seek was just performed and we should measure on next heartbeat
    pub fn mark_seek_performed(&mut self) {
        self.awaiting_measurement = true;
        tracing::debug!("Seek calibrator: marked awaiting measurement");
    }

    /// Called on each heartbeat. If we were awaiting a measurement (just seeked),
    /// this records the drift and updates calibration. Returns true if a measurement was taken.
    ///
    /// - Negative drift = we're behind host → need MORE offset
    /// - Positive drift = we're ahead of host → need LESS offset
    pub fn measure_if_pending(&mut self, drift_ms: i64) -> bool {
        if !self.awaiting_measurement {
            return false;
        }

        // Clear the flag - we only measure once per seek
        self.awaiting_measurement = false;

        // Calculate ideal offset for this measurement
        let ideal_offset = self.offset_ms - drift_ms as f64;

        // Determine alpha based on drift magnitude
        // Large drifts (outliers) get much smaller weight - we learn slowly from them
        let is_outlier = drift_ms.abs() > MAX_CALIBRATION_DRIFT_MS;

        self.sample_count = self.sample_count.saturating_add(1);

        let alpha = if is_outlier {
            // Outlier: learn very slowly (but still learn!)
            tracing::debug!(
                "Seek calibrator: outlier drift={:+}ms, using damped alpha={}",
                drift_ms,
                OUTLIER_ALPHA
            );
            OUTLIER_ALPHA
        } else if self.sample_count <= 5 {
            0.4 // Faster initial calibration
        } else {
            EMA_ALPHA
        };

        // EMA update
        self.offset_ms = alpha * ideal_offset + (1.0 - alpha) * self.offset_ms;

        // Clamp to bounds
        self.offset_ms = self.offset_ms.clamp(MIN_SEEK_OFFSET_MS as f64, MAX_SEEK_OFFSET_MS as f64);

        // Record sample (mark outliers as "rejected" meaning damped weight)
        self.record_sample(CalibrationSample {
            drift_ms,
            ideal_offset_ms: ideal_offset.round() as i64,
            new_offset_ms: self.offset_ms.round() as u64,
            rejected: is_outlier,
        });

        tracing::debug!(
            "Seek calibrator: measured drift={:+}ms, ideal={}ms, new_offset={}ms (samples={}, outlier={})",
            drift_ms,
            ideal_offset.round(),
            self.offset_ms.round(),
            self.sample_count,
            is_outlier
        );

        true
    }

    /// Record a sample to history, maintaining max size
    fn record_sample(&mut self, sample: CalibrationSample) {
        self.sample_history.push(sample);
        if self.sample_history.len() > MAX_SAMPLE_HISTORY {
            self.sample_history.remove(0);
        }
    }

    /// Get recent calibration sample history (newest last)
    pub fn sample_history(&self) -> &[CalibrationSample] {
        &self.sample_history
    }

    /// Reset calibration (e.g., when joining a new room)
    pub fn reset(&mut self) {
        self.offset_ms = DEFAULT_SEEK_OFFSET_MS as f64;
        self.sample_count = 0;
        self.awaiting_measurement = false;
        self.sample_history.clear();
    }
}

impl Default for SeekCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for SeekCalibrator
pub type SharedSeekCalibrator = Arc<RwLock<SeekCalibrator>>;

/// Create a new shared seek calibrator
pub fn new_shared_calibrator() -> SharedSeekCalibrator {
    Arc::new(RwLock::new(SeekCalibrator::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_offset() {
        let calibrator = SeekCalibrator::new();
        assert_eq!(calibrator.offset_ms(), DEFAULT_SEEK_OFFSET_MS);
    }

    #[test]
    fn test_no_update_without_pending() {
        let mut calibrator = SeekCalibrator::new();
        let initial = calibrator.offset_ms();

        // Without marking seek performed, measure_if_pending should do nothing
        let updated = calibrator.measure_if_pending(-200);
        assert!(!updated);
        assert_eq!(calibrator.offset_ms(), initial);
    }

    #[test]
    fn test_behind_increases_offset() {
        let mut calibrator = SeekCalibrator::new();
        let initial = calibrator.offset_ms();

        // Mark seek performed, then measure
        calibrator.mark_seek_performed();
        let updated = calibrator.measure_if_pending(-200); // We're behind by 200ms

        assert!(updated);
        assert!(calibrator.offset_ms() > initial);
    }

    #[test]
    fn test_only_one_measurement_per_seek() {
        let mut calibrator = SeekCalibrator::new();

        // Mark seek performed
        calibrator.mark_seek_performed();

        // First measurement should update
        let updated1 = calibrator.measure_if_pending(-200);
        assert!(updated1);
        let after_first = calibrator.offset_ms();

        // Second measurement without new seek should NOT update
        let updated2 = calibrator.measure_if_pending(-200);
        assert!(!updated2);
        assert_eq!(calibrator.offset_ms(), after_first);
    }

    #[test]
    fn test_ahead_decreases_offset() {
        let mut calibrator = SeekCalibrator::new();

        // Prime with some samples
        for _ in 0..10 {
            calibrator.mark_seek_performed();
            calibrator.measure_if_pending(0);
        }
        let initial = calibrator.offset_ms();

        // We're ahead by 200ms
        calibrator.mark_seek_performed();
        calibrator.measure_if_pending(200);

        // Offset should decrease
        assert!(calibrator.offset_ms() < initial);
    }

    #[test]
    fn test_clamping() {
        let mut calibrator = SeekCalibrator::new();

        // Try to push way below minimum
        for _ in 0..100 {
            calibrator.mark_seek_performed();
            calibrator.measure_if_pending(1000); // Way ahead
        }
        assert!(calibrator.offset_ms() >= MIN_SEEK_OFFSET_MS);

        // Try to push way above maximum
        calibrator.reset();
        for _ in 0..100 {
            calibrator.mark_seek_performed();
            calibrator.measure_if_pending(-5000); // Way behind
        }
        assert!(calibrator.offset_ms() <= MAX_SEEK_OFFSET_MS);
    }

    #[test]
    fn test_convergence() {
        let mut calibrator = SeekCalibrator::new();

        // Simulate: true Cider latency is 700ms
        // Drift = current_offset - 700 (if offset < 700, we're behind)
        let true_latency: i64 = 700;

        for _ in 0..50 {
            let current_offset = calibrator.offset_ms() as i64;
            // Simulate drift based on how close we are to true latency
            let simulated_drift = current_offset - true_latency;

            calibrator.mark_seek_performed();
            calibrator.measure_if_pending(simulated_drift);
        }

        // Should converge close to 700ms
        let offset = calibrator.offset_ms();
        assert!(offset >= 650 && offset <= 750, "Expected ~700ms, got {}ms", offset);
    }
}
