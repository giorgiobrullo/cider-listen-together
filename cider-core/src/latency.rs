//! Latency tracking for peer-to-peer sync
//!
//! Measures round-trip time (RTT) to peers using ping/pong messages
//! and provides estimated one-way latency for position calculations.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Number of RTT samples to keep for averaging
const RTT_SAMPLE_COUNT: usize = 5;

/// Default latency estimate when no measurements exist (conservative for local network)
const DEFAULT_LATENCY_MS: u64 = 10;

/// A single pending ping awaiting response
struct PendingPing {
    sent_at: Instant,
}

/// RTT history for a single peer
struct PeerLatency {
    /// Recent RTT samples in milliseconds
    samples: Vec<u64>,
    /// Cached average RTT
    avg_rtt_ms: u64,
}

impl PeerLatency {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(RTT_SAMPLE_COUNT),
            avg_rtt_ms: DEFAULT_LATENCY_MS * 2, // RTT = 2 * one-way
        }
    }

    fn add_sample(&mut self, rtt_ms: u64) {
        if self.samples.len() >= RTT_SAMPLE_COUNT {
            self.samples.remove(0);
        }
        self.samples.push(rtt_ms);
        self.recalculate_average();
    }

    fn recalculate_average(&mut self) {
        if self.samples.is_empty() {
            self.avg_rtt_ms = DEFAULT_LATENCY_MS * 2;
            return;
        }
        let sum: u64 = self.samples.iter().sum();
        self.avg_rtt_ms = sum / self.samples.len() as u64;
    }

    /// Get estimated one-way latency (RTT / 2)
    fn one_way_latency_ms(&self) -> u64 {
        self.avg_rtt_ms / 2
    }
}

/// Tracks latency to peers in a room
#[derive(Default)]
pub struct LatencyTracker {
    /// Pending pings awaiting pong response, keyed by timestamp_ms
    pending_pings: HashMap<u64, PendingPing>,
    /// Latency data per peer
    peer_latencies: HashMap<String, PeerLatency>,
    /// Host peer ID (we only care about latency to host)
    host_peer_id: Option<String>,
}

impl LatencyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the host peer ID (latency to host is what matters for sync)
    pub fn set_host(&mut self, peer_id: String) {
        self.host_peer_id = Some(peer_id);
    }

    /// Clear all state (when leaving room)
    pub fn clear(&mut self) {
        self.pending_pings.clear();
        self.peer_latencies.clear();
        self.host_peer_id = None;
    }

    /// Create a ping to send. Returns the timestamp to include in the Ping message.
    pub fn create_ping(&mut self) -> u64 {
        let now = Instant::now();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.pending_pings.insert(
            timestamp_ms,
            PendingPing { sent_at: now },
        );

        // Clean up old pending pings (older than 10 seconds)
        self.pending_pings
            .retain(|_, p| p.sent_at.elapsed() < Duration::from_secs(10));

        timestamp_ms
    }

    /// Handle a pong response. Returns the measured RTT if valid.
    pub fn handle_pong(&mut self, from_peer: &str, original_timestamp_ms: u64) -> Option<u64> {
        let pending = self.pending_pings.remove(&original_timestamp_ms)?;
        let rtt_ms = pending.sent_at.elapsed().as_millis() as u64;

        // Record the RTT for this peer
        let peer_latency = self
            .peer_latencies
            .entry(from_peer.to_string())
            .or_insert_with(PeerLatency::new);
        peer_latency.add_sample(rtt_ms);

        tracing::debug!(
            "Latency to {}: RTT={}ms, avg={}ms, one-way={}ms",
            from_peer,
            rtt_ms,
            peer_latency.avg_rtt_ms,
            peer_latency.one_way_latency_ms()
        );

        Some(rtt_ms)
    }

    /// Get estimated one-way latency to the host in milliseconds.
    /// Returns DEFAULT_LATENCY_MS if no measurements exist.
    pub fn host_latency_ms(&self) -> u64 {
        if let Some(host_id) = &self.host_peer_id {
            if let Some(peer_latency) = self.peer_latencies.get(host_id) {
                return peer_latency.one_way_latency_ms();
            }
        }
        DEFAULT_LATENCY_MS
    }

    /// Get estimated one-way latency to a specific peer
    pub fn peer_latency_ms(&self, peer_id: &str) -> u64 {
        self.peer_latencies
            .get(peer_id)
            .map(|p| p.one_way_latency_ms())
            .unwrap_or(DEFAULT_LATENCY_MS)
    }
}

/// Thread-safe wrapper for LatencyTracker
pub type SharedLatencyTracker = Arc<RwLock<LatencyTracker>>;

/// Create a new shared latency tracker
pub fn new_shared_tracker() -> SharedLatencyTracker {
    Arc::new(RwLock::new(LatencyTracker::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_tracker_basics() {
        let mut tracker = LatencyTracker::new();
        tracker.set_host("host123".to_string());

        // No measurements yet - should return default
        assert_eq!(tracker.host_latency_ms(), DEFAULT_LATENCY_MS);

        // Simulate a ping/pong with 50ms RTT
        let ts = tracker.create_ping();
        std::thread::sleep(Duration::from_millis(50));
        let rtt = tracker.handle_pong("host123", ts);

        assert!(rtt.is_some());
        let measured_rtt = rtt.unwrap();
        assert!(measured_rtt >= 50); // At least 50ms

        // One-way should be roughly half
        let one_way = tracker.host_latency_ms();
        assert!(one_way >= 25);
    }

    #[test]
    fn test_averaging() {
        let mut tracker = LatencyTracker::new();

        // Add multiple samples manually via handle_pong simulation
        let peer_latency = tracker
            .peer_latencies
            .entry("peer1".to_string())
            .or_insert_with(PeerLatency::new);

        peer_latency.add_sample(100);
        peer_latency.add_sample(200);
        peer_latency.add_sample(150);

        // Average should be (100+200+150)/3 = 150, one-way = 75
        assert_eq!(peer_latency.avg_rtt_ms, 150);
        assert_eq!(peer_latency.one_way_latency_ms(), 75);
    }
}
