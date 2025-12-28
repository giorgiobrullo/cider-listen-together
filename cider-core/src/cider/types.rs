//! Types for Cider API responses

use serde::{Deserialize, Serialize};

/// Response wrapper for most Cider API endpoints
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    pub status: String,
    #[serde(flatten)]
    pub data: T,
}

/// Artwork information for a track
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    pub width: u32,
    pub height: u32,
    pub url: String,
}

/// Currently playing track information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    /// Unique identifier for the song
    #[serde(default)]
    pub play_params: Option<PlayParams>,

    /// Song name
    pub name: String,

    /// Artist name
    pub artist_name: String,

    /// Album name
    pub album_name: String,

    /// Artwork information
    pub artwork: Artwork,

    /// Total duration in milliseconds
    pub duration_in_millis: u64,

    /// Current playback position in seconds
    #[serde(default)]
    pub current_playback_time: f64,

    /// Remaining time in seconds
    #[serde(default)]
    pub remaining_time: f64,

    /// Genre names
    #[serde(default)]
    pub genre_names: Vec<String>,

    /// Track number on album
    #[serde(default)]
    pub track_number: u32,

    /// Release date
    #[serde(default)]
    pub release_date: Option<String>,

    /// Whether the song has lyrics
    #[serde(default)]
    pub has_lyrics: bool,

    /// Whether in user's favorites
    #[serde(default)]
    pub in_favorites: bool,

    /// Whether in user's library
    #[serde(default)]
    pub in_library: bool,

    /// Shuffle mode (0 = off, 1 = on)
    #[serde(default)]
    pub shuffle_mode: u8,

    /// Repeat mode (0 = off, 1 = repeat one, 2 = repeat all)
    #[serde(default)]
    pub repeat_mode: u8,

    /// Apple Music URL
    #[serde(default)]
    pub url: Option<String>,
}

impl NowPlaying {
    /// Get the song ID from play params
    pub fn song_id(&self) -> Option<&str> {
        self.play_params.as_ref().map(|p| p.id.as_str())
    }

    /// Get current playback position in milliseconds
    pub fn current_position_ms(&self) -> u64 {
        (self.current_playback_time * 1000.0) as u64
    }

    /// Get the full-resolution artwork URL
    pub fn artwork_url(&self, size: u32) -> String {
        self.artwork
            .url
            .replace("{w}", &size.to_string())
            .replace("{h}", &size.to_string())
            .replace("/{w}x{h}", &format!("/{}x{}", size, size))
    }
}

/// Play parameters for a track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayParams {
    pub id: String,
    pub kind: String,
}

/// Playback state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub position_ms: u64,
    pub timestamp: u64,
}

/// Response for is-playing endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct IsPlayingResponse {
    pub is_playing: bool,
}

/// Response for now-playing endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct NowPlayingResponse {
    pub info: NowPlaying,
}

/// Response for volume endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct VolumeResponse {
    pub volume: f32,
}

/// Response for repeat-mode endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct RepeatModeResponse {
    pub value: u8,
}

/// Response for shuffle-mode endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct ShuffleModeResponse {
    pub value: u8,
}

/// Response for autoplay endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct AutoplayResponse {
    pub value: bool,
}

/// Request body for play-url endpoint
#[derive(Debug, Clone, Serialize)]
pub struct PlayUrlRequest {
    pub url: String,
}

/// Request body for play-item endpoint
#[derive(Debug, Clone, Serialize)]
pub struct PlayItemRequest {
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
}

/// Request body for seek endpoint
#[derive(Debug, Clone, Serialize)]
pub struct SeekRequest {
    pub position: f64,
}

/// Request body for volume endpoint
#[derive(Debug, Clone, Serialize)]
pub struct VolumeRequest {
    pub volume: f32,
}

/// Request body for rating endpoint
#[derive(Debug, Clone, Serialize)]
pub struct RatingRequest {
    pub rating: i8,
}
