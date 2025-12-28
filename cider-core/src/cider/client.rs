//! Cider API HTTP Client

use std::time::Duration;
use reqwest::Client;
use thiserror::Error;
use tracing::{debug, warn, instrument};

use super::types::*;

/// Default Cider API port
pub const DEFAULT_PORT: u16 = 10767;

/// Default connection timeout (short since it's localhost)
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(1);

/// Default request timeout (short since it's localhost)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Errors that can occur when communicating with Cider
#[derive(Debug, Error)]
pub enum CiderError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Cider is not running or not reachable")]
    NotReachable,

    #[error("Invalid API token")]
    Unauthorized,

    #[error("No track currently playing")]
    NothingPlaying,

    #[error("API error: {0}")]
    Api(String),
}

/// Client for interacting with Cider's REST API
#[derive(Debug, Clone)]
pub struct CiderClient {
    http: Client,
    base_url: String,
    api_token: Option<String>,
}

impl CiderClient {
    /// Create a new CiderClient with default settings (localhost:10767)
    pub fn new() -> Self {
        Self::with_port(DEFAULT_PORT)
    }

    /// Create a new CiderClient with a custom port
    pub fn with_port(port: u16) -> Self {
        let http = Client::builder()
            .connect_timeout(CONNECTION_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            // Limit connection pool to avoid stale connections
            .pool_max_idle_per_host(2)
            .pool_idle_timeout(Duration::from_secs(10))
            // Disable keep-alive to ensure fresh connections
            .tcp_keepalive(None)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            // Use 127.0.0.1 explicitly to avoid IPv6 issues
            base_url: format!("http://127.0.0.1:{}", port),
            api_token: None,
        }
    }

    /// Set the API token for authentication
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    /// Build a request with optional authentication
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/api/v1/playback{}", self.base_url, path);
        let mut req = self.http.request(method, &url);

        if let Some(token) = &self.api_token {
            req = req.header("apitoken", token);
        }

        req
    }

    /// Check if Cider is active and reachable
    #[instrument(skip(self), fields(base_url = %self.base_url))]
    pub async fn is_active(&self) -> Result<(), CiderError> {
        debug!("Checking Cider connection");

        let resp = self.request(reqwest::Method::GET, "/active")
            .send()
            .await
            .map_err(|e| {
                warn!("Connection error: {:?}", e);
                if e.is_connect() {
                    CiderError::Api(format!("Connection refused ({})", e))
                } else if e.is_timeout() {
                    CiderError::Api("Connection timed out".to_string())
                } else {
                    CiderError::Api(format!("Network error ({})", e))
                }
            })?;

        debug!("Response status: {}", resp.status());

        match resp.status().as_u16() {
            200 | 204 => Ok(()),
            401 | 403 => Err(CiderError::Unauthorized),
            _ => Err(CiderError::Api(format!("Unexpected response (HTTP {})", resp.status().as_u16()))),
        }
    }

    /// Check if music is currently playing
    pub async fn is_playing(&self) -> Result<bool, CiderError> {
        let resp: ApiResponse<IsPlayingResponse> = self
            .request(reqwest::Method::GET, "/is-playing")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.is_playing)
    }

    /// Get the currently playing track (returns None if nothing is playing)
    pub async fn now_playing(&self) -> Result<Option<NowPlaying>, CiderError> {
        let resp = self
            .request(reqwest::Method::GET, "/now-playing")
            .send()
            .await?;

        // Handle case where nothing is playing
        if resp.status() == 404 || resp.status() == 204 {
            return Ok(None);
        }

        // Try to parse the response - if it fails, assume nothing is playing
        match resp.json::<ApiResponse<NowPlayingResponse>>().await {
            Ok(data) => Ok(Some(data.data.info)),
            Err(_) => Ok(None),
        }
    }

    /// Resume playback
    pub async fn play(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Pause playback
    pub async fn pause(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/pause")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Toggle play/pause
    pub async fn play_pause(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/playpause")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Stop playback
    pub async fn stop(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/stop")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Skip to next track
    pub async fn next(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/next")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Go to previous track
    pub async fn previous(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/previous")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Seek to a position in the current track
    ///
    /// # Arguments
    /// * `position_secs` - Position in seconds to seek to
    pub async fn seek(&self, position_secs: f64) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/seek")
            .json(&SeekRequest {
                position: position_secs,
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Seek to a position in milliseconds
    pub async fn seek_ms(&self, position_ms: u64) -> Result<(), CiderError> {
        self.seek(position_ms as f64 / 1000.0).await
    }

    /// Play a track by its Apple Music URL
    pub async fn play_url(&self, url: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-url")
            .json(&PlayUrlRequest {
                url: url.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Play a track by type and ID
    ///
    /// # Arguments
    /// * `item_type` - Type of item (e.g., "songs", "albums", "playlists")
    /// * `id` - Apple Music ID of the item
    pub async fn play_item(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-item")
            .json(&PlayItemRequest {
                item_type: item_type.to_string(),
                id: id.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Add a track to play next
    pub async fn play_next(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-next")
            .json(&PlayItemRequest {
                item_type: item_type.to_string(),
                id: id.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Add a track to play later (end of queue)
    pub async fn play_later(&self, item_type: &str, id: &str) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/play-later")
            .json(&PlayItemRequest {
                item_type: item_type.to_string(),
                id: id.to_string(),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Get current volume (0.0 to 1.0)
    pub async fn get_volume(&self) -> Result<f32, CiderError> {
        let resp: ApiResponse<VolumeResponse> = self
            .request(reqwest::Method::GET, "/volume")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.volume)
    }

    /// Set volume (0.0 to 1.0)
    pub async fn set_volume(&self, volume: f32) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/volume")
            .json(&VolumeRequest {
                volume: volume.clamp(0.0, 1.0),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Add current track to library
    pub async fn add_to_library(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/add-to-library")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Set rating for current track (-1 = dislike, 0 = unset, 1 = like)
    pub async fn set_rating(&self, rating: i8) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/set-rating")
            .json(&RatingRequest {
                rating: rating.clamp(-1, 1),
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Get repeat mode (0 = off, 1 = repeat one, 2 = repeat all)
    pub async fn get_repeat_mode(&self) -> Result<u8, CiderError> {
        let resp: ApiResponse<RepeatModeResponse> = self
            .request(reqwest::Method::GET, "/repeat-mode")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.value)
    }

    /// Toggle repeat mode
    pub async fn toggle_repeat(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/toggle-repeat")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Get shuffle mode (0 = off, 1 = on)
    pub async fn get_shuffle_mode(&self) -> Result<u8, CiderError> {
        let resp: ApiResponse<ShuffleModeResponse> = self
            .request(reqwest::Method::GET, "/shuffle-mode")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.value)
    }

    /// Toggle shuffle mode
    pub async fn toggle_shuffle(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/toggle-shuffle")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Get autoplay status
    pub async fn get_autoplay(&self) -> Result<bool, CiderError> {
        let resp: ApiResponse<AutoplayResponse> = self
            .request(reqwest::Method::GET, "/autoplay")
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data.value)
    }

    /// Toggle autoplay
    pub async fn toggle_autoplay(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/toggle-autoplay")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Clear the queue
    pub async fn clear_queue(&self) -> Result<(), CiderError> {
        self.request(reqwest::Method::POST, "/queue/clear-queue")
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

impl Default for CiderClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = CiderClient::new();
        assert_eq!(client.base_url, "http://localhost:10767");

        let client_with_token = CiderClient::new().with_token("test-token");
        assert_eq!(client_with_token.api_token, Some("test-token".to_string()));
    }
}
