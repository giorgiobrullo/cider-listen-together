//! Metrics tracking for the relay server

use chrono::{DateTime, Local};
use std::collections::VecDeque;

/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 100;

/// A log entry for the dashboard
#[derive(Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Connection,
    Relay,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Connection => "CONN",
            LogLevel::Relay => "RELAY",
        }
    }
}

/// Server metrics
pub struct Metrics {
    /// Server start time
    pub start_time: DateTime<Local>,

    /// Our peer ID
    pub peer_id: Option<String>,

    /// Public IP address
    pub public_ip: Option<String>,

    /// TCP port
    pub tcp_port: u16,

    /// QUIC port
    pub quic_port: u16,

    /// TCP port reachable from internet
    pub tcp_reachable: Option<bool>,

    /// Current number of connected peers
    pub connected_peers: usize,

    /// Total connections since start
    pub total_connections: u64,

    /// Peak simultaneous connections
    pub peak_connections: usize,

    /// Active relay reservations
    pub active_reservations: usize,

    /// Total relay reservations since start
    pub total_reservations: u64,

    /// Active relay circuits
    pub active_circuits: usize,

    /// Total relay circuits since start
    pub total_circuits: u64,

    /// Bytes relayed (approximate)
    pub bytes_relayed: u64,

    /// Connected peer IDs (for display)
    pub peer_list: Vec<PeerInfo>,

    /// Log entries
    pub logs: VecDeque<LogEntry>,

    /// Server status
    pub status: ServerStatus,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct PeerInfo {
    pub peer_id: String,
    pub protocol: Option<String>,
    pub connected_at: DateTime<Local>,
    pub has_reservation: bool,
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum ServerStatus {
    Starting,
    Running,
    Error,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            start_time: Local::now(),
            peer_id: None,
            public_ip: None,
            tcp_port: 4001,
            quic_port: 4001,
            tcp_reachable: None,
            connected_peers: 0,
            total_connections: 0,
            peak_connections: 0,
            active_reservations: 0,
            total_reservations: 0,
            active_circuits: 0,
            total_circuits: 0,
            bytes_relayed: 0,
            peer_list: Vec::new(),
            logs: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            status: ServerStatus::Starting,
        }
    }

    /// Add a log entry
    pub fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        if self.logs.len() >= MAX_LOG_ENTRIES {
            self.logs.pop_front();
        }
        self.logs.push_back(LogEntry {
            timestamp: Local::now(),
            level,
            message: message.into(),
        });
    }

    /// Record a new connection (only counts unique peers)
    pub fn connection_established(&mut self, peer_id: String, protocol: Option<String>) {
        // Check if this peer is already connected (multiple transports to same peer)
        if self.peer_list.iter().any(|p| p.peer_id == peer_id) {
            // Already connected via another transport, don't double count
            return;
        }

        self.connected_peers += 1;
        self.total_connections += 1;
        if self.connected_peers > self.peak_connections {
            self.peak_connections = self.connected_peers;
        }

        self.peer_list.push(PeerInfo {
            peer_id: peer_id.clone(),
            protocol,
            connected_at: Local::now(),
            has_reservation: false,
        });

        let short_id = truncate_peer_id(&peer_id);
        self.log(LogLevel::Connection, format!("Connected: {}", short_id));
    }

    /// Record a disconnection (only if peer was tracked)
    pub fn connection_closed(&mut self, peer_id: &str) {
        // Find the peer and check if they had a reservation before removing
        let peer_info = self.peer_list.iter().find(|p| p.peer_id == peer_id);

        let Some(peer) = peer_info else {
            // Peer wasn't tracked, nothing to clean up
            return;
        };

        // If peer had a reservation, decrement active count
        if peer.has_reservation {
            self.active_reservations = self.active_reservations.saturating_sub(1);
        }

        self.connected_peers = self.connected_peers.saturating_sub(1);
        self.peer_list.retain(|p| p.peer_id != peer_id);

        let short_id = truncate_peer_id(peer_id);
        self.log(LogLevel::Connection, format!("Disconnected: {}", short_id));
    }

    /// Record a relay reservation
    pub fn reservation_accepted(&mut self, peer_id: &str) {
        // Check if peer already has a reservation (avoid double counting)
        let already_has_reservation = self
            .peer_list
            .iter()
            .find(|p| p.peer_id == peer_id)
            .map(|p| p.has_reservation)
            .unwrap_or(false);

        if already_has_reservation {
            // Reservation renewal, don't increment active count
            let short_id = truncate_peer_id(peer_id);
            self.log(LogLevel::Relay, format!("Reservation renewed: {}", short_id));
            return;
        }

        self.active_reservations += 1;
        self.total_reservations += 1;

        // Mark peer as having reservation
        if let Some(peer) = self.peer_list.iter_mut().find(|p| p.peer_id == peer_id) {
            peer.has_reservation = true;
        }

        let short_id = truncate_peer_id(peer_id);
        self.log(LogLevel::Relay, format!("Reservation: {}", short_id));
    }

    /// Record a relay circuit
    pub fn circuit_established(&mut self, src: &str, dst: &str) {
        self.active_circuits += 1;
        self.total_circuits += 1;

        let src_short = truncate_peer_id(src);
        let dst_short = truncate_peer_id(dst);
        self.log(LogLevel::Relay, format!("Circuit: {} → {}", src_short, dst_short));
    }

    /// Record circuit closed
    pub fn circuit_closed(&mut self) {
        self.active_circuits = self.active_circuits.saturating_sub(1);
    }

    /// Update peer protocol info (logging is handled by caller)
    pub fn peer_identified(&mut self, peer_id: &str, protocol: String) {
        if let Some(peer) = self.peer_list.iter_mut().find(|p| p.peer_id == peer_id) {
            peer.protocol = Some(protocol);
        }
    }

    /// Get uptime as formatted string
    pub fn uptime(&self) -> String {
        let duration = Local::now().signed_duration_since(self.start_time);
        let secs = duration.num_seconds();

        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            format!("{}h {}m", hours, mins)
        }
    }
}

/// Truncate peer ID for display (show first and last few chars)
pub fn truncate_peer_id(peer_id: &str) -> String {
    if peer_id.len() > 16 {
        format!("{}...{}", &peer_id[..8], &peer_id[peer_id.len()-4..])
    } else {
        peer_id.to_string()
    }
}
