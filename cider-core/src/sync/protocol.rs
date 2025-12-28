//! Sync Protocol Messages

use serde::{Deserialize, Serialize};

/// Information about a track for sync purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    /// Apple Music song ID
    pub song_id: String,
    /// Song name
    pub name: String,
    /// Artist name
    pub artist: String,
    /// Album name
    pub album: String,
    /// Artwork URL
    pub artwork_url: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Participant in a listening room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    /// Unique peer ID
    pub peer_id: String,
    /// Display name chosen by user
    pub display_name: String,
    /// Whether this participant is the current host
    pub is_host: bool,
}

/// Current playback state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackInfo {
    /// Whether music is playing
    pub is_playing: bool,
    /// Current position in milliseconds
    pub position_ms: u64,
    /// Timestamp when this state was captured (monotonic clock)
    pub timestamp_ms: u64,
}

/// Messages exchanged between peers for synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    // === Room Management ===
    /// Full room state (sent to new joiners)
    RoomState {
        room_code: String,
        host_peer_id: String,
        participants: Vec<Participant>,
        current_track: Option<TrackInfo>,
        playback: PlaybackInfo,
    },

    /// Request to join a room
    JoinRequest { display_name: String },

    /// Response to join request
    JoinResponse {
        accepted: bool,
        room_code: Option<String>,
        reason: Option<String>,
    },

    /// Notification that someone joined
    ParticipantJoined(Participant),

    /// Notification that someone left
    ParticipantLeft { peer_id: String },

    /// Host is transferring control to another peer
    TransferHost { new_host_peer_id: String },

    // === Playback Commands (from host) ===
    /// Start or resume playback
    Play {
        track: TrackInfo,
        position_ms: u64,
        timestamp_ms: u64,
    },

    /// Pause playback
    Pause { position_ms: u64, timestamp_ms: u64 },

    /// Seek to position
    Seek { position_ms: u64, timestamp_ms: u64 },

    /// Track changed
    TrackChange {
        track: TrackInfo,
        position_ms: u64,
        timestamp_ms: u64,
    },

    // === Clock Synchronization ===
    /// Ping for measuring round-trip time
    Ping { sent_at_ms: u64 },

    /// Pong response for RTT calculation
    Pong {
        ping_sent_at_ms: u64,
        received_at_ms: u64,
    },

    // === Periodic Sync ===
    /// Heartbeat with current playback state (sent by host periodically)
    Heartbeat {
        track_id: Option<String>,
        playback: PlaybackInfo,
    },
}

impl SyncMessage {
    /// Check if this is a playback command that requires host privileges
    pub fn requires_host(&self) -> bool {
        matches!(
            self,
            SyncMessage::Play { .. }
                | SyncMessage::Pause { .. }
                | SyncMessage::Seek { .. }
                | SyncMessage::TrackChange { .. }
                | SyncMessage::TransferHost { .. }
        )
    }
}
