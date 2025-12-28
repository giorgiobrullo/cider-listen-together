//! FFI types exposed via uniffi

use crate::seek_calibrator::CalibrationSample as InternalCalibrationSample;
use crate::sync::{Participant as InternalParticipant, PlaybackInfo, RoomState as InternalRoomState, TrackInfo as InternalTrackInfo};

/// Error types exposed via FFI
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("Cider is not reachable")]
    CiderNotReachable,

    #[error("Cider API error: {0}")]
    CiderApiError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Not in a room")]
    NotInRoom,

    #[error("Already in a room")]
    AlreadyInRoom,

    #[error("Not the host")]
    NotHost,

    #[error("Join timeout - room not found or host not reachable")]
    JoinTimeout,
}

/// Track information exposed via FFI
#[derive(Debug, Clone, uniffi::Record)]
pub struct TrackInfo {
    pub song_id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub artwork_url: String,
    pub duration_ms: u64,
    pub position_ms: u64,
}

impl From<InternalTrackInfo> for TrackInfo {
    fn from(t: InternalTrackInfo) -> Self {
        Self {
            song_id: t.song_id,
            name: t.name,
            artist: t.artist,
            album: t.album,
            artwork_url: t.artwork_url,
            duration_ms: t.duration_ms,
            position_ms: 0, // Will be updated by playback state
        }
    }
}

impl From<&crate::cider::NowPlaying> for TrackInfo {
    fn from(np: &crate::cider::NowPlaying) -> Self {
        Self {
            song_id: np.song_id().unwrap_or("").to_string(),
            name: np.name.clone(),
            artist: np.artist_name.clone(),
            album: np.album_name.clone(),
            artwork_url: np.artwork_url(600),
            duration_ms: np.duration_in_millis,
            position_ms: np.current_position_ms(),
        }
    }
}

impl From<&TrackInfo> for InternalTrackInfo {
    fn from(t: &TrackInfo) -> Self {
        Self {
            song_id: t.song_id.clone(),
            name: t.name.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            artwork_url: t.artwork_url.clone(),
            duration_ms: t.duration_ms,
        }
    }
}

/// Participant exposed via FFI
#[derive(Debug, Clone, uniffi::Record)]
pub struct Participant {
    pub peer_id: String,
    pub display_name: String,
    pub is_host: bool,
}

impl From<&InternalParticipant> for Participant {
    fn from(p: &InternalParticipant) -> Self {
        Self {
            peer_id: p.peer_id.clone(),
            display_name: p.display_name.clone(),
            is_host: p.is_host,
        }
    }
}

/// Playback state exposed via FFI
#[derive(Debug, Clone, uniffi::Record)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_ms: u64,
    pub timestamp_ms: u64,
}

impl From<&PlaybackInfo> for PlaybackState {
    fn from(p: &PlaybackInfo) -> Self {
        Self {
            is_playing: p.is_playing,
            position_ms: p.position_ms,
            timestamp_ms: p.timestamp_ms,
        }
    }
}

/// Current playback info (for polling) exposed via FFI
#[derive(Debug, Clone, uniffi::Record)]
pub struct CurrentPlayback {
    pub track: Option<TrackInfo>,
    pub is_playing: bool,
}

/// Room state exposed via FFI
#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomState {
    pub room_code: String,
    pub local_peer_id: String,
    pub host_peer_id: String,
    pub participants: Vec<Participant>,
    pub current_track: Option<TrackInfo>,
    pub playback: PlaybackState,
}

impl From<&InternalRoomState> for RoomState {
    fn from(r: &InternalRoomState) -> Self {
        Self {
            room_code: r.room_code.clone(),
            local_peer_id: r.local_peer_id.clone(),
            host_peer_id: r.host_peer_id.clone(),
            participants: r.participant_list().into_iter().map(Participant::from).collect(),
            current_track: r.current_track.as_ref().map(|t| TrackInfo::from(t.clone())),
            playback: PlaybackState::from(&r.playback),
        }
    }
}

/// A calibration sample for debug display
#[derive(Debug, Clone, uniffi::Record)]
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

impl From<&InternalCalibrationSample> for CalibrationSample {
    fn from(s: &InternalCalibrationSample) -> Self {
        Self {
            drift_ms: s.drift_ms,
            ideal_offset_ms: s.ideal_offset_ms,
            new_offset_ms: s.new_offset_ms,
            rejected: s.rejected,
        }
    }
}

/// Sync status for debug display
#[derive(Debug, Clone, uniffi::Record)]
pub struct SyncStatus {
    /// Drift in milliseconds (positive = ahead of host, negative = behind)
    pub drift_ms: i64,
    /// One-way latency to host in milliseconds
    pub latency_ms: u64,
    /// Time elapsed since host's heartbeat timestamp
    pub elapsed_ms: u64,
    /// Calibrated seek offset for Cider buffer latency
    pub seek_offset_ms: u64,
    /// Whether calibrator is waiting to measure after a seek
    pub calibration_pending: bool,
    /// What the next calibration sample would be (if pending and not outlier)
    /// None if not pending or would be rejected as outlier
    pub next_calibration_sample: Option<i64>,
    /// Recent calibration samples (newest last)
    pub sample_history: Vec<CalibrationSample>,
}

/// Callback interface for session events
#[uniffi::export(callback_interface)]
pub trait SessionCallback: Send + Sync {
    fn on_room_state_changed(&self, state: RoomState);
    fn on_track_changed(&self, track: Option<TrackInfo>);
    fn on_playback_changed(&self, playback: PlaybackState);
    fn on_participant_joined(&self, participant: Participant);
    fn on_participant_left(&self, peer_id: String);
    fn on_room_ended(&self, reason: String);
    fn on_error(&self, message: String);
    fn on_connected(&self);
    fn on_disconnected(&self);
    /// Called periodically with sync status (listeners only)
    fn on_sync_status(&self, status: SyncStatus);
}

/// Get current time in milliseconds since UNIX epoch
pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
