//! Simple signaling via ntfy.sh for room discovery
//!
//! Uses the free ntfy.sh pub/sub service to exchange peer addresses.
//! No signup required, works immediately over the internet.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

const NTFY_BASE_URL: &str = "https://ntfy.sh";

/// Message published to signaling channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalingMessage {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub room_code: String,
}

/// Signaling client for room discovery
pub struct SignalingClient {
    client: Client,
}

impl SignalingClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Normalize room code for topic naming - strips hyphens and lowercases
    fn normalize_room_code(room_code: &str) -> String {
        room_code
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase()
    }

    /// Publish our addresses to the room's signaling channel
    pub async fn publish_room(
        &self,
        room_code: &str,
        peer_id: &str,
        addresses: Vec<String>,
    ) -> Result<(), String> {
        let normalized = Self::normalize_room_code(room_code);
        let topic = format!("cider-together-{}", normalized);
        let url = format!("{}/{}", NTFY_BASE_URL, topic);

        let msg = SignalingMessage {
            peer_id: peer_id.to_string(),
            addresses,
            room_code: room_code.to_string(),
        };

        let body = serde_json::to_string(&msg).map_err(|e| e.to_string())?;

        info!("Signaling: Publishing room {} (topic: {}) to ntfy.sh", room_code, topic);

        self.client
            .post(&url)
            .header("Title", format!("Room {}", room_code))
            .header("Tags", "musical_note")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to publish to signaling: {}", e))?;

        info!("Signaling: Room {} published successfully", room_code);
        Ok(())
    }

    /// Poll for peers in a room (gets recent messages)
    pub async fn poll_room(&self, room_code: &str) -> Result<Vec<SignalingMessage>, String> {
        let normalized = Self::normalize_room_code(room_code);
        let topic = format!("cider-together-{}", normalized);
        // Use the JSON endpoint with poll=1 to get cached messages
        let url = format!("{}/{}/json?poll=1&since=5m", NTFY_BASE_URL, topic);

        debug!("Signaling: Polling room {} (topic: {})", room_code, topic);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to poll signaling: {}", e))?;

        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // ntfy returns newline-delimited JSON
        let mut messages = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // Parse ntfy message wrapper
            if let Ok(ntfy_msg) = serde_json::from_str::<serde_json::Value>(line) {
                // The actual message is in the "message" field
                if let Some(message_str) = ntfy_msg.get("message").and_then(|m| m.as_str()) {
                    if let Ok(sig_msg) = serde_json::from_str::<SignalingMessage>(message_str) {
                        messages.push(sig_msg);
                    }
                }
            }
        }

        if !messages.is_empty() {
            info!("Signaling: Found {} peers in room {}", messages.len(), room_code);
        }

        Ok(messages)
    }
}

impl Default for SignalingClient {
    fn default() -> Self {
        Self::new()
    }
}
