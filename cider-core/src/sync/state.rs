//! Room State Management

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::protocol::{Participant, PlaybackInfo, TrackInfo};

/// Current state of the room
#[derive(Debug, Clone)]
pub struct RoomState {
    /// Room code for sharing
    pub room_code: String,
    /// Our peer ID
    pub local_peer_id: String,
    /// Current host's peer ID
    pub host_peer_id: String,
    /// All participants including ourselves
    pub participants: HashMap<String, Participant>,
    /// Currently playing track
    pub current_track: Option<TrackInfo>,
    /// Current playback state
    pub playback: PlaybackInfo,
    /// When we last received a heartbeat from host
    pub last_heartbeat: Instant,
}

impl RoomState {
    /// Create a new room state for a host
    pub fn new_as_host(room_code: String, local_peer_id: String, display_name: String) -> Self {
        let mut participants = HashMap::new();
        participants.insert(
            local_peer_id.clone(),
            Participant {
                peer_id: local_peer_id.clone(),
                display_name,
                is_host: true,
            },
        );

        Self {
            room_code,
            local_peer_id: local_peer_id.clone(),
            host_peer_id: local_peer_id,
            participants,
            current_track: None,
            playback: PlaybackInfo {
                is_playing: false,
                position_ms: 0,
                timestamp_ms: 0,
            },
            last_heartbeat: Instant::now(),
        }
    }

    /// Check if we are the host
    pub fn is_host(&self) -> bool {
        self.local_peer_id == self.host_peer_id
    }

    /// Get list of participants (host first, then others sorted by display name)
    pub fn participant_list(&self) -> Vec<&Participant> {
        let mut list: Vec<&Participant> = self.participants.values().collect();
        list.sort_by(|a, b| {
            // Host always first
            match (a.is_host, b.is_host) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                // Among non-hosts, sort by display name
                _ => a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()),
            }
        });
        list
    }

    /// Add a participant
    pub fn add_participant(&mut self, participant: Participant) {
        self.participants
            .insert(participant.peer_id.clone(), participant);
    }

    /// Remove a participant
    pub fn remove_participant(&mut self, peer_id: &str) -> Option<Participant> {
        self.participants.remove(peer_id)
    }

    /// Transfer host to another peer
    pub fn transfer_host(&mut self, new_host_peer_id: &str) -> bool {
        // Check if new host exists
        if !self.participants.contains_key(new_host_peer_id) {
            return false;
        }

        let old_host_peer_id = self.host_peer_id.clone();

        // Remove host status from old host
        if let Some(old_host) = self.participants.get_mut(&old_host_peer_id) {
            old_host.is_host = false;
        }

        // Set new host
        if let Some(new_host) = self.participants.get_mut(new_host_peer_id) {
            new_host.is_host = true;
        }

        self.host_peer_id = new_host_peer_id.to_string();
        true
    }

    /// Update playback state
    pub fn update_playback(&mut self, playback: PlaybackInfo) {
        self.playback = playback;
        self.last_heartbeat = Instant::now();
    }

    /// Update current track
    pub fn update_track(&mut self, track: Option<TrackInfo>) {
        self.current_track = track;
    }

    /// Check if heartbeat is stale (host might be disconnected)
    pub fn is_heartbeat_stale(&self, timeout: Duration) -> bool {
        self.last_heartbeat.elapsed() > timeout
    }
}

/// Represents the room we're in (or not)
#[derive(Debug)]
pub enum Room {
    /// Not in any room
    None,
    /// Creating a room (waiting for network setup)
    Creating { display_name: String },
    /// Joining a room (waiting for response)
    Joining {
        room_code: String,
        display_name: String,
    },
    /// In an active room
    Active(RoomState),
}

impl Room {
    /// Check if we're in an active room
    pub fn is_active(&self) -> bool {
        matches!(self, Room::Active(_))
    }

    /// Check if we're in any room-related state (creating, joining, or active)
    pub fn is_busy(&self) -> bool {
        !matches!(self, Room::None)
    }

    /// Get the active room state if we're in one
    pub fn state(&self) -> Option<&RoomState> {
        match self {
            Room::Active(state) => Some(state),
            _ => None,
        }
    }

    /// Get mutable reference to active room state
    pub fn state_mut(&mut self) -> Option<&mut RoomState> {
        match self {
            Room::Active(state) => Some(state),
            _ => None,
        }
    }
}

impl Default for Room {
    fn default() -> Self {
        Room::None
    }
}
