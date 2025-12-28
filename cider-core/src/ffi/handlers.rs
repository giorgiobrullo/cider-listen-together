//! Network event and sync message handlers

use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::cider::CiderClient;
use crate::latency::SharedLatencyTracker;
use crate::network::{NetworkEvent, NetworkHandle};
use crate::sync::{Participant as InternalParticipant, Room, SyncMessage};

use super::types::{Participant, PlaybackState, RoomState, SessionCallback, TrackInfo};

/// Handle a network event
pub async fn handle_network_event(
    event: NetworkEvent,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    cider: &Arc<RwLock<CiderClient>>,
    network_handle: &Arc<RwLock<Option<NetworkHandle>>>,
    latency_tracker: &SharedLatencyTracker,
    local_peer_id: &str,
) {
    match event {
        NetworkEvent::Ready { peer_id } => {
            info!("Network ready with peer ID: {}", peer_id);
        }

        NetworkEvent::PeerSubscribed { peer_id } => {
            info!("Peer subscribed to room: {}", peer_id);

            // If we're the host, send current room state to the new peer
            let room_guard = room.read().unwrap();
            if let Some(state) = room_guard.state() {
                if state.is_host() {
                    // Broadcast room state so new peer can join
                    if let Some(handle) = network_handle.read().unwrap().as_ref() {
                        let msg = SyncMessage::RoomState {
                            room_code: state.room_code.clone(),
                            host_peer_id: state.host_peer_id.clone(),
                            participants: state.participant_list().iter().map(|p| InternalParticipant {
                                peer_id: p.peer_id.clone(),
                                display_name: p.display_name.clone(),
                                is_host: p.is_host,
                            }).collect(),
                            current_track: state.current_track.clone(),
                            playback: state.playback.clone(),
                        };
                        let _ = handle.broadcast(msg);
                    }
                }
            }
        }

        NetworkEvent::PeerUnsubscribed { peer_id } => {
            info!("Peer left room: {}", peer_id);

            let mut room_guard = room.write().unwrap();
            if let Some(state) = room_guard.state_mut() {
                // Check if the leaving peer is the host
                let is_host_leaving = state.host_peer_id == peer_id;
                let we_are_host = state.is_host();

                if state.remove_participant(&peer_id).is_some() {
                    if let Some(cb) = callback.read().unwrap().as_ref() {
                        cb.on_participant_left(peer_id.clone());

                        if is_host_leaving && !we_are_host {
                            // Host left and we're a listener - room is ending
                            info!("Host left the room, ending session for listener");
                            cb.on_room_ended("Host left the room".to_string());

                            // Pause playback since host is gone
                            let cider_client = cider.read().unwrap().clone();
                            tokio::spawn(async move {
                                let _ = cider_client.pause().await;
                            });

                            // Clear room state after notifying
                            drop(room_guard);
                            *room.write().unwrap() = Room::None;
                            return;
                        } else {
                            cb.on_room_state_changed(RoomState::from(&*state));
                        }
                    }
                }
            }
        }

        NetworkEvent::Message { from, message } => {
            handle_sync_message(from, message, room, callback, cider, network_handle, latency_tracker, local_peer_id).await;
        }

        NetworkEvent::Error(e) => {
            warn!("Network error: {}", e);
            if let Some(cb) = callback.read().unwrap().as_ref() {
                cb.on_error(e);
            }
        }
    }
}

/// Handle a sync message from another peer
pub async fn handle_sync_message(
    from: String,
    message: SyncMessage,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    cider: &Arc<RwLock<CiderClient>>,
    network_handle: &Arc<RwLock<Option<NetworkHandle>>>,
    latency_tracker: &SharedLatencyTracker,
    local_peer_id: &str,
) {
    match message {
        SyncMessage::JoinRequest { display_name } => {
            handle_join_request(from, display_name, room, callback, network_handle);
        }

        SyncMessage::RoomState {
            room_code,
            host_peer_id,
            participants,
            current_track,
            playback,
        } => {
            handle_room_state(
                room_code,
                host_peer_id,
                participants,
                current_track,
                playback,
                room,
                callback,
                cider,
                network_handle,
                latency_tracker,
                local_peer_id,
            ).await;
        }

        SyncMessage::ParticipantJoined(participant) => {
            handle_participant_joined(participant, room, callback);
        }

        SyncMessage::ParticipantLeft { peer_id } => {
            handle_participant_left(peer_id, room, callback);
        }

        SyncMessage::TransferHost { new_host_peer_id } => {
            handle_transfer_host(new_host_peer_id, room, callback);
        }

        SyncMessage::Play { track, position_ms, .. } => {
            handle_play(track, position_ms, room, cider).await;
        }

        SyncMessage::Pause { position_ms, .. } => {
            handle_pause(position_ms, room, cider).await;
        }

        SyncMessage::Seek { position_ms, .. } => {
            handle_seek(position_ms, room, cider).await;
        }

        SyncMessage::TrackChange { track, position_ms, timestamp_ms } => {
            handle_track_change(track, position_ms, timestamp_ms, room, callback, cider).await;
        }

        SyncMessage::Heartbeat { track_id: _, playback } => {
            handle_heartbeat(playback, room, callback, cider, latency_tracker).await;
        }

        // Ping/Pong for latency measurement
        SyncMessage::Ping { sent_at_ms } => {
            // Respond with Pong containing the original timestamp
            if let Some(handle) = network_handle.read().unwrap().as_ref() {
                let pong = SyncMessage::Pong {
                    ping_sent_at_ms: sent_at_ms,
                    received_at_ms: super::types::current_time_ms(),
                };
                let _ = handle.broadcast(pong);
            }
        }

        SyncMessage::Pong { ping_sent_at_ms, .. } => {
            // Record RTT measurement
            let mut tracker = latency_tracker.write().unwrap();
            if let Some(rtt) = tracker.handle_pong(&from, ping_sent_at_ms) {
                debug!("Measured RTT to {}: {}ms", from, rtt);
            }
        }

        SyncMessage::JoinResponse { .. } => {}
    }
}

fn handle_join_request(
    from: String,
    display_name: String,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    network_handle: &Arc<RwLock<Option<NetworkHandle>>>,
) {
    // Only host handles join requests
    let mut room_guard = room.write().unwrap();
    if let Some(state) = room_guard.state_mut() {
        if state.is_host() {
            info!("Join request from {} ({})", display_name, from);

            // Add participant
            state.add_participant(InternalParticipant {
                peer_id: from.clone(),
                display_name: display_name.clone(),
                is_host: false,
            });

            // Notify callback
            if let Some(cb) = callback.read().unwrap().as_ref() {
                cb.on_participant_joined(Participant {
                    peer_id: from.clone(),
                    display_name: display_name.clone(),
                    is_host: false,
                });
                cb.on_room_state_changed(RoomState::from(&*state));
            }

            // Broadcast updated room state
            if let Some(handle) = network_handle.read().unwrap().as_ref() {
                let msg = SyncMessage::RoomState {
                    room_code: state.room_code.clone(),
                    host_peer_id: state.host_peer_id.clone(),
                    participants: state.participant_list().iter().map(|p| InternalParticipant {
                        peer_id: p.peer_id.clone(),
                        display_name: p.display_name.clone(),
                        is_host: p.is_host,
                    }).collect(),
                    current_track: state.current_track.clone(),
                    playback: state.playback.clone(),
                };
                let _ = handle.broadcast(msg);
            }
        }
    }
}

async fn handle_room_state(
    room_code: String,
    host_peer_id: String,
    participants: Vec<InternalParticipant>,
    current_track: Option<crate::sync::TrackInfo>,
    playback: crate::sync::PlaybackInfo,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    cider: &Arc<RwLock<CiderClient>>,
    network_handle: &Arc<RwLock<Option<NetworkHandle>>>,
    latency_tracker: &SharedLatencyTracker,
    local_peer_id: &str,
) {
    use crate::sync::RoomState as InternalRoomState;

    // Set the host in latency tracker for accurate sync
    {
        let mut tracker = latency_tracker.write().unwrap();
        tracker.set_host(host_peer_id.clone());
    }

    // Track info for syncing after we release the lock
    // (song_id, position_ms, timestamp_ms, is_playing)
    let track_to_sync: Option<(String, u64, u64, bool)>;
    let was_joining: bool;
    let display_name_for_join: String;

    {
        let mut room_guard = room.write().unwrap();

        // Check if we're joining or already in room
        let should_update = match &*room_guard {
            Room::Joining { room_code: our_code, .. } => room_code == *our_code,
            Room::Active(state) => room_code == state.room_code && !state.is_host(),
            _ => false,
        };

        if !should_update {
            return;
        }

        let display_name = match &*room_guard {
            Room::Joining { display_name, .. } => display_name.clone(),
            Room::Active(state) => state.participants.get(&state.local_peer_id)
                .map(|p| p.display_name.clone())
                .unwrap_or_else(|| "Listener".to_string()),
            _ => "Listener".to_string(),
        };
        display_name_for_join = display_name.clone();

        info!("Received room state from host");

        // Capture track info before updating state (including timestamp for accurate sync)
        track_to_sync = current_track.as_ref().map(|t| {
            (t.song_id.clone(), playback.position_ms, playback.timestamp_ms, playback.is_playing)
        });

        let mut new_state = InternalRoomState::new_as_host(
            room_code.clone(),
            local_peer_id.to_string(),
            display_name,
        );
        new_state.host_peer_id = host_peer_id;
        new_state.current_track = current_track;
        new_state.playback = playback;

        // Clear default self-participant and add actual participants
        new_state.participants.clear();
        for p in participants {
            new_state.add_participant(p);
        }

        was_joining = matches!(&*room_guard, Room::Joining { .. });
        *room_guard = Room::Active(new_state);

        if let Some(cb) = callback.read().unwrap().as_ref() {
            if let Some(state) = room_guard.state() {
                cb.on_room_state_changed(RoomState::from(state));
                if was_joining {
                    cb.on_connected();
                }
            }
        }
    }

    // Send JoinRequest after transitioning to Active to ensure host adds us
    // (the initial JoinRequest during Joining state may not have reached the host yet)
    if was_joining {
        if let Some(handle) = network_handle.read().unwrap().as_ref() {
            info!("Sending JoinRequest after joining: {}", display_name_for_join);
            let join_msg = SyncMessage::JoinRequest {
                display_name: display_name_for_join,
            };
            let _ = handle.broadcast(join_msg);
        }
    }

    // Sync Cider to host's track when joining
    if was_joining {
        if let Some((song_id, position_ms, timestamp_ms, is_playing)) = track_to_sync {
            info!("Syncing Cider to host's track: {} at {}ms", song_id, position_ms);
            let cider_client = cider.read().unwrap().clone();

            // Start playing the track
            let _ = cider_client.play_item("songs", &song_id).await;

            // Poll until track is actually loaded (max 5 seconds)
            let max_wait = Duration::from_secs(5);
            let poll_interval = Duration::from_millis(100);
            let start = std::time::Instant::now();

            loop {
                if start.elapsed() > max_wait {
                    warn!("Timeout waiting for track to load, seeking anyway");
                    break;
                }

                if let Ok(Some(np)) = cider_client.now_playing().await {
                    if np.song_id() == Some(&song_id) {
                        info!("Track loaded after {:?}", start.elapsed());
                        break;
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }

            // Calculate actual position accounting for elapsed time since heartbeat
            let now = super::types::current_time_ms();
            let elapsed_since_heartbeat = now.saturating_sub(timestamp_ms);
            let actual_position = if is_playing {
                position_ms + elapsed_since_heartbeat
            } else {
                position_ms
            };

            info!("Seeking to adjusted position: {}ms (original: {}ms, elapsed: {}ms)",
                actual_position, position_ms, elapsed_since_heartbeat);

            let _ = cider_client.seek_ms(actual_position).await;
        }
    }
}

fn handle_participant_joined(
    participant: InternalParticipant,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
) {
    let mut room_guard = room.write().unwrap();
    if let Some(state) = room_guard.state_mut() {
        state.add_participant(InternalParticipant {
            peer_id: participant.peer_id.clone(),
            display_name: participant.display_name.clone(),
            is_host: participant.is_host,
        });

        if let Some(cb) = callback.read().unwrap().as_ref() {
            cb.on_participant_joined(Participant {
                peer_id: participant.peer_id,
                display_name: participant.display_name,
                is_host: participant.is_host,
            });
            cb.on_room_state_changed(RoomState::from(&*state));
        }
    }
}

fn handle_participant_left(
    peer_id: String,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
) {
    let mut room_guard = room.write().unwrap();
    if let Some(state) = room_guard.state_mut() {
        state.remove_participant(&peer_id);

        if let Some(cb) = callback.read().unwrap().as_ref() {
            cb.on_participant_left(peer_id);
            cb.on_room_state_changed(RoomState::from(&*state));
        }
    }
}

fn handle_transfer_host(
    new_host_peer_id: String,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
) {
    let mut room_guard = room.write().unwrap();
    if let Some(state) = room_guard.state_mut() {
        state.transfer_host(&new_host_peer_id);

        if let Some(cb) = callback.read().unwrap().as_ref() {
            cb.on_room_state_changed(RoomState::from(&*state));
        }
    }
}

async fn handle_play(
    track: crate::sync::TrackInfo,
    position_ms: u64,
    room: &Arc<RwLock<Room>>,
    cider: &Arc<RwLock<CiderClient>>,
) {
    // Non-host: sync to host's playback
    let should_sync = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| !s.is_host()).unwrap_or(false)
    };

    if should_sync {
        let cider_client = cider.read().unwrap().clone();
        let song_id = track.song_id.clone();
        // Play the same track at the same position
        let _ = cider_client.play_item("songs", &song_id).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = cider_client.seek_ms(position_ms).await;
        let _ = cider_client.play().await;
    }
}

async fn handle_pause(
    position_ms: u64,
    room: &Arc<RwLock<Room>>,
    cider: &Arc<RwLock<CiderClient>>,
) {
    let should_sync = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| !s.is_host()).unwrap_or(false)
    };

    if should_sync {
        let cider_client = cider.read().unwrap().clone();
        let _ = cider_client.pause().await;
        let _ = cider_client.seek_ms(position_ms).await;
    }
}

async fn handle_seek(
    position_ms: u64,
    room: &Arc<RwLock<Room>>,
    cider: &Arc<RwLock<CiderClient>>,
) {
    let should_sync = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| !s.is_host()).unwrap_or(false)
    };

    if should_sync {
        let cider_client = cider.read().unwrap().clone();
        let _ = cider_client.seek_ms(position_ms).await;
    }
}

async fn handle_track_change(
    track: crate::sync::TrackInfo,
    position_ms: u64,
    timestamp_ms: u64,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    cider: &Arc<RwLock<CiderClient>>,
) {
    let is_host = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| s.is_host()).unwrap_or(false)
    };

    if !is_host {
        let cider_client = cider.read().unwrap().clone();
        let song_id = track.song_id.clone();
        let _ = cider_client.play_item("songs", &song_id).await;

        // Poll until track is actually loaded (max 5 seconds)
        let max_wait = Duration::from_secs(5);
        let poll_interval = Duration::from_millis(100);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > max_wait {
                warn!("TrackChange: timeout waiting for track to load");
                break;
            }

            if let Ok(Some(np)) = cider_client.now_playing().await {
                if np.song_id() == Some(&song_id) {
                    info!("TrackChange: track loaded after {:?}", start.elapsed());
                    break;
                }
            }

            tokio::time::sleep(poll_interval).await;
        }

        // Calculate actual position accounting for elapsed time
        let now = super::types::current_time_ms();
        let elapsed = now.saturating_sub(timestamp_ms);
        let actual_position = position_ms + elapsed;

        info!("TrackChange: seeking to {}ms (original: {}ms, elapsed: {}ms)",
            actual_position, position_ms, elapsed);

        let _ = cider_client.seek_ms(actual_position).await;
    }

    // Update local state
    let mut room_guard = room.write().unwrap();
    if let Some(state) = room_guard.state_mut() {
        state.update_track(Some(track.clone()));
        if let Some(cb) = callback.read().unwrap().as_ref() {
            cb.on_track_changed(Some(TrackInfo::from(track)));
        }
    }
}

/// Maximum position drift (in ms) before we re-sync the listener
const DRIFT_THRESHOLD_MS: u64 = 3000;

async fn handle_heartbeat(
    playback: crate::sync::PlaybackInfo,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    cider: &Arc<RwLock<CiderClient>>,
    latency_tracker: &SharedLatencyTracker,
) {
    // Check if we're a listener and need to sync
    let should_sync = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| !s.is_host()).unwrap_or(false)
    };

    if should_sync {
        // Get estimated one-way latency to host
        let latency_ms = latency_tracker.read().unwrap().host_latency_ms();

        // Calculate expected position based on heartbeat timestamp + latency compensation
        let now = super::types::current_time_ms();
        let elapsed_since_heartbeat = now.saturating_sub(playback.timestamp_ms);
        let expected_position = if playback.is_playing {
            // Add latency to account for network delay
            playback.position_ms + elapsed_since_heartbeat + latency_ms
        } else {
            playback.position_ms
        };

        // Get current Cider playback state to check drift
        let cider_client = cider.read().unwrap().clone();

        // Check current position from now_playing
        if let Ok(Some(np)) = cider_client.now_playing().await {
            let current_position = np.current_position_ms();

            // Check if we're drifted too far from expected position
            let drift = if current_position > expected_position {
                current_position - expected_position
            } else {
                expected_position - current_position
            };

            if drift > DRIFT_THRESHOLD_MS {
                info!(
                    "Heartbeat: position drift {}ms exceeds threshold, re-syncing (expected: {}ms, actual: {}ms)",
                    drift, expected_position, current_position
                );
                let _ = cider_client.seek_ms(expected_position).await;
            }
        }

        // Also sync play/pause state
        if let Ok(is_currently_playing) = cider_client.is_playing().await {
            if playback.is_playing && !is_currently_playing {
                info!("Heartbeat: host is playing but we're paused, resuming");
                let _ = cider_client.play().await;
            } else if !playback.is_playing && is_currently_playing {
                info!("Heartbeat: host is paused but we're playing, pausing");
                let _ = cider_client.pause().await;
            }
        }
    }

    // Update local state
    let mut room_guard = room.write().unwrap();
    if let Some(state) = room_guard.state_mut() {
        if !state.is_host() {
            state.update_playback(playback.clone());

            if let Some(cb) = callback.read().unwrap().as_ref() {
                cb.on_playback_changed(PlaybackState::from(&playback));
            }
        }
    }
}
