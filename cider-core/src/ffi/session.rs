//! Session implementation for FFI

use std::sync::{Arc, Once, RwLock};
use std::time::Duration;
use tokio::runtime::Runtime;
use tracing::{debug, info, warn};

use crate::cider::{CiderClient, CiderError as CiderApiError};
use crate::latency::{self, SharedLatencyTracker};
use crate::network::{NetworkHandle, NetworkManager, RoomCode};
use crate::sync::{PlaybackInfo, Room, RoomState as InternalRoomState, SyncMessage};

use super::handlers::handle_network_event;
use super::types::*;

static TRACING_INIT: Once = Once::new();

/// Main session interface
#[derive(uniffi::Object)]
pub struct Session {
    runtime: Runtime,
    cider: Arc<RwLock<CiderClient>>,
    room: Arc<RwLock<Room>>,
    callback: Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    network_handle: Arc<RwLock<Option<NetworkHandle>>>,
    local_peer_id: Arc<RwLock<Option<String>>>,
    /// Handle for cancelling the host broadcast loop
    host_broadcast_cancel: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// Last broadcasted track ID (for detecting changes)
    last_broadcast_track_id: Arc<RwLock<Option<String>>>,
    /// Latency tracker for measuring RTT to host
    latency_tracker: SharedLatencyTracker,
    /// Handle for cancelling the listener ping loop
    listener_ping_cancel: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

#[uniffi::export]
impl Session {
    /// Create a new session
    #[uniffi::constructor]
    pub fn new() -> Self {
        // Initialize tracing once
        TRACING_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_ansi(false)  // Disable colors for Xcode console
                .with_target(false)  // Cleaner output
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive("cider_core=debug".parse().unwrap())
                        .add_directive("libp2p_mdns=info".parse().unwrap())
                        .add_directive("libp2p_gossipsub=info".parse().unwrap())
                        .add_directive("hyper_util=off".parse().unwrap())
                        .add_directive("reqwest=off".parse().unwrap())
                        .add_directive("hyper=off".parse().unwrap()),
                )
                .with_writer(std::io::stderr)
                .init();
        });

        info!("Initializing cider-core session");

        let runtime = Runtime::new().expect("Failed to create tokio runtime");

        Self {
            runtime,
            cider: Arc::new(RwLock::new(CiderClient::new())),
            room: Arc::new(RwLock::new(Room::None)),
            callback: Arc::new(RwLock::new(None)),
            network_handle: Arc::new(RwLock::new(None)),
            local_peer_id: Arc::new(RwLock::new(None)),
            host_broadcast_cancel: Arc::new(RwLock::new(None)),
            last_broadcast_track_id: Arc::new(RwLock::new(None)),
            latency_tracker: latency::new_shared_tracker(),
            listener_ping_cancel: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the Cider API token
    pub fn set_cider_token(&self, token: Option<String>) {
        let mut cider = self.cider.write().unwrap();
        *cider = if let Some(t) = token {
            CiderClient::new().with_token(t)
        } else {
            CiderClient::new()
        };
    }

    /// Set the event callback
    pub fn set_callback(&self, callback: Box<dyn SessionCallback>) {
        let mut cb = self.callback.write().unwrap();
        *cb = Some(Arc::from(callback));
    }

    /// Check if Cider is reachable
    pub fn check_cider_connection(&self) -> Result<(), CoreError> {
        debug!("Checking Cider connection...");
        let cider = self.cider.read().unwrap();
        let result = self.runtime.block_on(async {
            cider.is_active().await.map_err(|e| match e {
                CiderApiError::Unauthorized => CoreError::CiderApiError("Invalid API token".to_string()),
                CiderApiError::Api(msg) => CoreError::CiderApiError(msg),
                CiderApiError::Http(e) => CoreError::NetworkError(e.to_string()),
                _ => CoreError::CiderApiError(e.to_string()),
            })
        });
        match &result {
            Ok(()) => info!("Cider connection OK"),
            Err(e) => warn!("Cider connection failed: {:?}", e),
        }
        result
    }

    /// Get the currently playing track from Cider
    pub fn get_now_playing(&self) -> Result<Option<TrackInfo>, CoreError> {
        let cider = self.cider.read().unwrap();
        let result = self.runtime.block_on(async {
            match cider.now_playing().await {
                Ok(Some(np)) => Ok(Some(TrackInfo::from(&np))),
                Ok(None) => Ok(None),
                Err(CiderApiError::NotReachable) => Err(CoreError::CiderNotReachable),
                Err(e) => Err(CoreError::CiderApiError(e.to_string())),
            }
        });
        match &result {
            Ok(Some(track)) => debug!("Now playing: {} - {} ({}ms)", track.name, track.artist, track.position_ms),
            Ok(None) => debug!("Nothing playing"),
            Err(e) => warn!("get_now_playing failed: {:?}", e),
        }
        result
    }

    /// Check if Cider is currently playing
    pub fn get_is_playing(&self) -> Result<bool, CoreError> {
        let cider = self.cider.read().unwrap();
        let result = self.runtime.block_on(async {
            match cider.is_playing().await {
                Ok(playing) => Ok(playing),
                Err(CiderApiError::NotReachable) => Err(CoreError::CiderNotReachable),
                Err(e) => Err(CoreError::CiderApiError(e.to_string())),
            }
        });
        match &result {
            Ok(playing) => debug!("is_playing: {}", playing),
            Err(e) => warn!("get_is_playing failed: {:?}", e),
        }
        result
    }

    /// Get playback state (track info + is_playing) in a single call
    pub fn get_playback_state(&self) -> Result<CurrentPlayback, CoreError> {
        let cider = self.cider.read().unwrap();
        let result = self.runtime.block_on(async {
            // Run both requests concurrently
            let (track_result, playing_result) = tokio::join!(
                cider.now_playing(),
                cider.is_playing()
            );

            let track = match track_result {
                Ok(Some(np)) => Some(TrackInfo::from(&np)),
                Ok(None) => None,
                Err(CiderApiError::NotReachable) => return Err(CoreError::CiderNotReachable),
                Err(e) => return Err(CoreError::CiderApiError(e.to_string())),
            };

            let is_playing = match playing_result {
                Ok(playing) => playing,
                Err(CiderApiError::NotReachable) => return Err(CoreError::CiderNotReachable),
                Err(e) => return Err(CoreError::CiderApiError(e.to_string())),
            };

            Ok(CurrentPlayback { track, is_playing })
        });

        match &result {
            Ok(CurrentPlayback { track: Some(t), is_playing }) => debug!("Playback: {} - {} ({}ms), playing={}", t.name, t.artist, t.position_ms, is_playing),
            Ok(CurrentPlayback { track: None, is_playing }) => debug!("Playback: nothing playing, playing={}", is_playing),
            Err(e) => warn!("get_playback_state failed: {:?}", e),
        }
        result
    }

    /// Create a new room (become host)
    pub fn create_room(&self, display_name: String) -> Result<String, CoreError> {
        {
            let room = self.room.read().unwrap();
            if room.is_active() {
                return Err(CoreError::AlreadyInRoom);
            }
        }

        // Start the network if not already running
        let (handle, peer_id) = self.ensure_network_running()?;

        // Generate room code
        let room_code = RoomCode::random();
        let room_code_str = room_code.as_str().to_string();

        // Tell network to create the room
        handle
            .create_room(&room_code_str)
            .map_err(|e| CoreError::NetworkError(e.to_string()))?;

        // Create local room state
        let state = InternalRoomState::new_as_host(
            room_code_str.clone(),
            peer_id.clone(),
            display_name,
        );

        {
            let mut room = self.room.write().unwrap();
            *room = Room::Active(state);
        }

        // Notify callback
        if let Some(cb) = self.callback.read().unwrap().as_ref() {
            let room = self.room.read().unwrap();
            if let Some(state) = room.state() {
                cb.on_room_state_changed(RoomState::from(state));
            }
        }

        // Start host broadcast loop
        self.start_host_broadcast_loop();

        info!("Created room: {}", room_code);
        Ok(room_code.to_string())
    }

    /// Join an existing room
    pub fn join_room(&self, room_code: String, display_name: String) -> Result<(), CoreError> {
        {
            let room = self.room.read().unwrap();
            if room.is_active() {
                return Err(CoreError::AlreadyInRoom);
            }
        }

        // Validate room code
        let code = RoomCode::parse(&room_code)
            .ok_or_else(|| CoreError::NetworkError("Invalid room code".to_string()))?;
        let room_code_str = code.as_str().to_string();

        // Start the network if not already running
        let (handle, _) = self.ensure_network_running()?;

        // Set room to joining state
        {
            let mut room = self.room.write().unwrap();
            *room = Room::Joining {
                room_code: room_code_str.clone(),
                display_name: display_name.clone(),
            };
        }

        // Tell network to join the room
        handle
            .join_room(&room_code_str)
            .map_err(|e| CoreError::NetworkError(e.to_string()))?;

        // Send join request with retry - the gossipsub mesh takes time to form
        // so the first few broadcasts might not reach the host
        let handle_clone = handle.clone();
        let display_name_clone = display_name.clone();
        let room_clone = Arc::clone(&self.room);
        let room_code_for_retry = room_code_str.clone();

        self.runtime.spawn(async move {
            // Wait a bit for mesh to form before first attempt
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Retry JoinRequest a few times until we're in the room
            for attempt in 1..=5 {
                // Check if we're still trying to join (not yet Active)
                let still_joining = {
                    let room = room_clone.read().unwrap();
                    matches!(&*room, Room::Joining { room_code, .. } if room_code == &room_code_for_retry)
                };

                if !still_joining {
                    debug!("No longer joining, stopping JoinRequest retries");
                    break;
                }

                debug!("Sending JoinRequest attempt {}/5", attempt);
                let join_msg = SyncMessage::JoinRequest {
                    display_name: display_name_clone.clone(),
                };
                let _ = handle_clone.broadcast(join_msg);

                // Wait before next retry
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        });

        // Start a timeout task - if no host responds, notify the user
        let room_clone = Arc::clone(&self.room);
        let callback_clone = Arc::clone(&self.callback);
        let room_code_for_timeout = room_code_str.clone();

        self.runtime.spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;

            // Check if we're still in joining state for this room
            let room = room_clone.read().unwrap();
            if let Room::Joining { room_code: rc, .. } = &*room {
                if rc == &room_code_for_timeout {
                    // No host found - notify the UI
                    warn!("No host found for room {} after timeout", room_code_for_timeout);

                    if let Some(cb) = callback_clone.read().unwrap().as_ref() {
                        cb.on_error(format!(
                            "Room {} not found",
                            room_code_for_timeout
                        ));
                    }
                }
            }
        });

        // Start ping loop to measure latency (host will be set when RoomState arrives)
        self.start_listener_ping_loop();

        info!("Joining room: {}", code);
        Ok(())
    }

    /// Leave the current room
    pub fn leave_room(&self) -> Result<(), CoreError> {
        {
            let room = self.room.read().unwrap();
            if !room.is_active() && !matches!(&*room, Room::Joining { .. }) {
                return Err(CoreError::NotInRoom);
            }
        }

        // Stop host broadcast loop if running
        self.stop_host_broadcast_loop();

        // Stop listener ping loop if running
        self.stop_listener_ping_loop();

        // Tell network to leave
        if let Some(handle) = self.network_handle.read().unwrap().as_ref() {
            let _ = handle.leave_room();
        }

        {
            let mut room = self.room.write().unwrap();
            *room = Room::None;
        }

        // Clear last broadcast track
        {
            let mut last_track = self.last_broadcast_track_id.write().unwrap();
            *last_track = None;
        }

        // Notify callback
        if let Some(cb) = self.callback.read().unwrap().as_ref() {
            cb.on_disconnected();
        }

        info!("Left room");
        Ok(())
    }

    /// Transfer host to another peer
    pub fn transfer_host(&self, peer_id: String) -> Result<(), CoreError> {
        let mut room = self.room.write().unwrap();
        let state = room.state_mut().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        if !state.transfer_host(&peer_id) {
            return Err(CoreError::NetworkError("Peer not found".to_string()));
        }

        // Broadcast transfer message
        if let Some(handle) = self.network_handle.read().unwrap().as_ref() {
            let msg = SyncMessage::TransferHost {
                new_host_peer_id: peer_id,
            };
            let _ = handle.broadcast(msg);
        }

        // Notify callback
        if let Some(cb) = self.callback.read().unwrap().as_ref() {
            cb.on_room_state_changed(RoomState::from(&*state));
        }

        Ok(())
    }

    /// Sync play command (host only)
    pub fn sync_play(&self) -> Result<(), CoreError> {
        let room = self.room.read().unwrap();
        let state = room.state().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        let cider = self.cider.read().unwrap();
        self.runtime.block_on(async {
            cider.play().await.map_err(|e| CoreError::CiderApiError(e.to_string()))
        })?;

        // Broadcast play command
        if let Some(handle) = self.network_handle.read().unwrap().as_ref() {
            if let Some(track) = &state.current_track {
                let msg = SyncMessage::Play {
                    track: track.clone(),
                    position_ms: state.playback.position_ms,
                    timestamp_ms: current_time_ms(),
                };
                let _ = handle.broadcast(msg);
            }
        }

        Ok(())
    }

    /// Sync pause command (host only)
    pub fn sync_pause(&self) -> Result<(), CoreError> {
        let room = self.room.read().unwrap();
        let state = room.state().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        let cider = self.cider.read().unwrap();
        self.runtime.block_on(async {
            cider.pause().await.map_err(|e| CoreError::CiderApiError(e.to_string()))
        })?;

        // Broadcast pause command
        if let Some(handle) = self.network_handle.read().unwrap().as_ref() {
            let msg = SyncMessage::Pause {
                position_ms: state.playback.position_ms,
                timestamp_ms: current_time_ms(),
            };
            let _ = handle.broadcast(msg);
        }

        Ok(())
    }

    /// Sync seek command (host only)
    pub fn sync_seek(&self, position_ms: u64) -> Result<(), CoreError> {
        let room = self.room.read().unwrap();
        let state = room.state().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        let cider = self.cider.read().unwrap();
        self.runtime.block_on(async {
            cider.seek_ms(position_ms).await.map_err(|e| CoreError::CiderApiError(e.to_string()))
        })?;

        // Broadcast seek command
        if let Some(handle) = self.network_handle.read().unwrap().as_ref() {
            let msg = SyncMessage::Seek {
                position_ms,
                timestamp_ms: current_time_ms(),
            };
            let _ = handle.broadcast(msg);
        }

        Ok(())
    }

    /// Sync next command (host only)
    pub fn sync_next(&self) -> Result<(), CoreError> {
        let room = self.room.read().unwrap();
        let state = room.state().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        let cider = self.cider.read().unwrap();
        self.runtime.block_on(async {
            cider.next().await.map_err(|e| CoreError::CiderApiError(e.to_string()))
        })
    }

    /// Sync previous command (host only)
    pub fn sync_previous(&self) -> Result<(), CoreError> {
        let room = self.room.read().unwrap();
        let state = room.state().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        let cider = self.cider.read().unwrap();
        self.runtime.block_on(async {
            cider.previous().await.map_err(|e| CoreError::CiderApiError(e.to_string()))
        })
    }

    /// Get current room state
    pub fn get_room_state(&self) -> Option<RoomState> {
        let room = self.room.read().unwrap();
        room.state().map(RoomState::from)
    }

    /// Check if we are the host
    pub fn is_host(&self) -> bool {
        let room = self.room.read().unwrap();
        room.state().map(|s| s.is_host()).unwrap_or(false)
    }

    /// Check if we are in a room
    pub fn is_in_room(&self) -> bool {
        let room = self.room.read().unwrap();
        room.is_active()
    }

    /// Broadcast current playback state to room (for host heartbeat)
    pub fn broadcast_playback(&self, track: Option<TrackInfo>, is_playing: bool, position_ms: u64) -> Result<(), CoreError> {
        let room = self.room.read().unwrap();
        let state = room.state().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        if let Some(handle) = self.network_handle.read().unwrap().as_ref() {
            let msg = SyncMessage::Heartbeat {
                track_id: track.as_ref().map(|t| t.song_id.clone()),
                playback: PlaybackInfo {
                    is_playing,
                    position_ms,
                    timestamp_ms: current_time_ms(),
                },
            };
            handle.broadcast(msg).map_err(|e| CoreError::NetworkError(e.to_string()))?;
        }

        Ok(())
    }

    /// Broadcast track change to room (for host when track changes)
    pub fn broadcast_track_change(&self, track: TrackInfo, position_ms: u64) -> Result<(), CoreError> {
        let mut room = self.room.write().unwrap();
        let state = room.state_mut().ok_or(CoreError::NotInRoom)?;

        if !state.is_host() {
            return Err(CoreError::NotHost);
        }

        // Update our local state with the new track
        let internal_track = crate::sync::TrackInfo {
            song_id: track.song_id.clone(),
            name: track.name.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            artwork_url: track.artwork_url.clone(),
            duration_ms: track.duration_ms,
        };
        state.update_track(Some(internal_track.clone()));

        // Broadcast the track change
        if let Some(handle) = self.network_handle.read().unwrap().as_ref() {
            let msg = SyncMessage::TrackChange {
                track: internal_track,
                position_ms,
                timestamp_ms: current_time_ms(),
            };
            handle.broadcast(msg).map_err(|e| CoreError::NetworkError(e.to_string()))?;
        }

        Ok(())
    }
}

impl Session {
    /// Ensure the network is running, start it if not
    fn ensure_network_running(&self) -> Result<(NetworkHandle, String), CoreError> {
        // Check if already running
        {
            let handle = self.network_handle.read().unwrap();
            if let Some(h) = handle.as_ref() {
                let peer_id = self.local_peer_id.read().unwrap().clone().unwrap();
                return Ok((h.clone(), peer_id));
            }
        }

        // Start the network
        let network_manager = NetworkManager::new()
            .map_err(|e| CoreError::NetworkError(e.to_string()))?;

        let (handle, mut event_rx) = self.runtime.block_on(async {
            network_manager.start()
        }).map_err(|e| CoreError::NetworkError(e.to_string()))?;

        let peer_id = handle.local_peer_id.clone();

        // Store the handle and peer ID
        {
            let mut h = self.network_handle.write().unwrap();
            *h = Some(handle.clone());
        }
        {
            let mut p = self.local_peer_id.write().unwrap();
            *p = Some(peer_id.clone());
        }

        // Spawn event handler task
        let room_clone = Arc::clone(&self.room);
        let callback_clone = Arc::clone(&self.callback);
        let cider_clone = Arc::clone(&self.cider);
        let network_handle_clone = Arc::clone(&self.network_handle);
        let latency_tracker_clone = Arc::clone(&self.latency_tracker);
        let local_peer_id = peer_id.clone();

        self.runtime.spawn(async move {
            while let Some(event) = event_rx.recv().await {
                handle_network_event(
                    event,
                    &room_clone,
                    &callback_clone,
                    &cider_clone,
                    &network_handle_clone,
                    &latency_tracker_clone,
                    &local_peer_id,
                ).await;
            }
        });

        Ok((handle, peer_id))
    }

    /// Start the host broadcast loop (polls Cider and broadcasts to listeners)
    fn start_host_broadcast_loop(&self) {
        // Stop any existing loop first
        self.stop_host_broadcast_loop();

        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();

        // Store cancel sender
        {
            let mut cancel = self.host_broadcast_cancel.write().unwrap();
            *cancel = Some(cancel_tx);
        }

        let cider = Arc::clone(&self.cider);
        let room = Arc::clone(&self.room);
        let network_handle = Arc::clone(&self.network_handle);
        let callback = Arc::clone(&self.callback);
        let last_track_id = Arc::clone(&self.last_broadcast_track_id);

        self.runtime.spawn(async move {
            info!("Host broadcast loop started");

            loop {
                // Check for cancellation
                if cancel_rx.try_recv().is_ok() {
                    info!("Host broadcast loop cancelled");
                    break;
                }

                // Check if we're still the host
                let is_host = {
                    let r = room.read().unwrap();
                    r.state().map(|s| s.is_host()).unwrap_or(false)
                };

                if !is_host {
                    debug!("No longer host, stopping broadcast loop");
                    break;
                }

                // Poll Cider for current playback
                let cider_client = cider.read().unwrap().clone();
                let playback_result = tokio::join!(
                    cider_client.now_playing(),
                    cider_client.is_playing()
                );

                if let (Ok(Some(np)), Ok(is_playing)) = playback_result {
                    let current_track_id: Option<String> = np.song_id().map(|s| s.to_string());
                    let position_ms = np.current_position_ms();

                    // Check if track changed
                    let track_changed = {
                        let last = last_track_id.read().unwrap();
                        last.as_ref() != current_track_id.as_ref()
                    };

                    // Build internal track info
                    let track = crate::sync::TrackInfo {
                        song_id: current_track_id.clone().unwrap_or_default(),
                        name: np.name.clone(),
                        artist: np.artist_name.clone(),
                        album: np.album_name.clone(),
                        artwork_url: np.artwork_url(600),
                        duration_ms: np.duration_in_millis,
                    };

                    if track_changed {
                        // Update last track ID
                        {
                            let mut last = last_track_id.write().unwrap();
                            *last = current_track_id.clone();
                        }

                        // Update room state
                        {
                            let mut r = room.write().unwrap();
                            if let Some(state) = r.state_mut() {
                                state.update_track(Some(track.clone()));
                                state.update_playback(PlaybackInfo {
                                    is_playing,
                                    position_ms,
                                    timestamp_ms: current_time_ms(),
                                });
                            }
                        }

                        // Broadcast track change
                        if let Some(handle) = network_handle.read().unwrap().as_ref() {
                            let msg = SyncMessage::TrackChange {
                                track: track.clone(),
                                position_ms,
                                timestamp_ms: current_time_ms(),
                            };
                            let _ = handle.broadcast(msg);
                        }

                        // Notify callback
                        if let Some(cb) = callback.read().unwrap().as_ref() {
                            cb.on_track_changed(Some(TrackInfo::from(track)));
                        }

                        debug!("Broadcasted track change: {}", np.name);
                    } else {
                        // Just broadcast heartbeat with position update
                        if let Some(handle) = network_handle.read().unwrap().as_ref() {
                            let msg = SyncMessage::Heartbeat {
                                track_id: current_track_id,
                                playback: PlaybackInfo {
                                    is_playing,
                                    position_ms,
                                    timestamp_ms: current_time_ms(),
                                },
                            };
                            let _ = handle.broadcast(msg);
                        }

                        // Update room playback state
                        {
                            let mut r = room.write().unwrap();
                            if let Some(state) = r.state_mut() {
                                state.update_playback(PlaybackInfo {
                                    is_playing,
                                    position_ms,
                                    timestamp_ms: current_time_ms(),
                                });
                            }
                        }
                    }
                }

                // Wait before next poll (1.5 seconds)
                tokio::time::sleep(Duration::from_millis(1500)).await;
            }

            info!("Host broadcast loop ended");
        });
    }

    /// Stop the host broadcast loop
    fn stop_host_broadcast_loop(&self) {
        let mut cancel = self.host_broadcast_cancel.write().unwrap();
        if let Some(tx) = cancel.take() {
            let _ = tx.send(());
        }
    }

    /// Start the listener ping loop (measures latency to peers)
    /// Host peer ID is set later when RoomState is received
    fn start_listener_ping_loop(&self) {
        // Stop any existing loop first
        self.stop_listener_ping_loop();

        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel();

        // Store cancel sender
        {
            let mut cancel = self.listener_ping_cancel.write().unwrap();
            *cancel = Some(cancel_tx);
        }

        let latency_tracker = Arc::clone(&self.latency_tracker);
        let network_handle = Arc::clone(&self.network_handle);
        let room = Arc::clone(&self.room);
        let callback = Arc::clone(&self.callback);
        let cider = Arc::clone(&self.cider);

        self.runtime.spawn(async move {
            debug!("Listener ping loop started");

            // Timeout for detecting host disconnect (15 seconds without heartbeat)
            let heartbeat_timeout = Duration::from_secs(15);

            loop {
                // Check for cancellation
                if cancel_rx.try_recv().is_ok() {
                    debug!("Listener ping loop cancelled");
                    break;
                }

                // Check if we're still in the room as a listener and if heartbeat is stale
                let (is_listener, is_stale) = {
                    let r = room.read().unwrap();
                    match r.state() {
                        Some(s) if !s.is_host() => (true, s.is_heartbeat_stale(heartbeat_timeout)),
                        _ => (false, false),
                    }
                };

                if !is_listener {
                    debug!("No longer listener, stopping ping loop");
                    break;
                }

                // Check for host timeout (force quit, crash, network loss)
                if is_stale {
                    warn!("Host heartbeat timeout - host may have disconnected");

                    // Pause playback
                    let cider_client = cider.read().unwrap().clone();
                    let _ = cider_client.pause().await;

                    // Notify callback
                    if let Some(cb) = callback.read().unwrap().as_ref() {
                        cb.on_room_ended("Host disconnected (timeout)".to_string());
                    }

                    // Clear room state
                    {
                        let mut r = room.write().unwrap();
                        *r = Room::None;
                    }

                    break;
                }

                // Create and send ping
                let timestamp = {
                    let mut tracker = latency_tracker.write().unwrap();
                    tracker.create_ping()
                };

                if let Some(handle) = network_handle.read().unwrap().as_ref() {
                    let ping = SyncMessage::Ping { sent_at_ms: timestamp };
                    let _ = handle.broadcast(ping);
                }

                // Wait before next ping (5 seconds)
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            debug!("Listener ping loop ended");
        });
    }

    /// Stop the listener ping loop
    fn stop_listener_ping_loop(&self) {
        let mut cancel = self.listener_ping_cancel.write().unwrap();
        if let Some(tx) = cancel.take() {
            let _ = tx.send(());
        }
        // Clear latency tracker
        let mut tracker = self.latency_tracker.write().unwrap();
        tracker.clear();
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
