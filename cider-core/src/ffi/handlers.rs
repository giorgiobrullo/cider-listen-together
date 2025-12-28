//! Network event and sync message handlers

use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::cider::CiderClient;
use crate::latency::SharedLatencyTracker;
use crate::network::{NetworkEvent, NetworkHandle};
use crate::seek_calibrator::SharedSeekCalibrator;
use crate::sync::{Participant as InternalParticipant, Room, SyncMessage};

use super::types::{CalibrationSample, Participant, PlaybackState, RoomState, SessionCallback, SyncStatus, TrackInfo};

/// Handle a network event
pub async fn handle_network_event(
    event: NetworkEvent,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    cider: &Arc<RwLock<CiderClient>>,
    network_handle: &Arc<RwLock<Option<NetworkHandle>>>,
    latency_tracker: &SharedLatencyTracker,
    seek_calibrator: &SharedSeekCalibrator,
    local_peer_id: &str,
) {
    match event {
        NetworkEvent::Ready { peer_id } => {
            info!("Network ready with peer ID: {}", peer_id);
        }

        NetworkEvent::PeerSubscribed { peer_id } => {
            info!("Peer subscribed to room: {}", peer_id);

            // If we're the host, add them as unknown listener and send room state
            let mut room_guard = room.write().unwrap();
            if let Some(state) = room_guard.state_mut() {
                if state.is_host() {
                    // Add as unknown listener immediately (will be updated if they send JoinRequest)
                    // Skip if it's ourselves or already known
                    if peer_id != state.local_peer_id && !state.participants.contains_key(&peer_id) {
                        info!("Adding unknown listener: {}", peer_id);
                        state.add_participant(InternalParticipant {
                            peer_id: peer_id.clone(),
                            display_name: "?".to_string(),
                            is_host: false,
                        });

                        // Notify UI about the new participant
                        if let Some(cb) = callback.read().unwrap().as_ref() {
                            cb.on_participant_joined(Participant {
                                peer_id: peer_id.clone(),
                                display_name: "?".to_string(),
                                is_host: false,
                            });
                        }
                    }

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
            handle_sync_message(from, message, room, callback, cider, network_handle, latency_tracker, seek_calibrator, local_peer_id).await;
        }

        NetworkEvent::Error(e) => {
            warn!("Network error: {}", e);
            if let Some(cb) = callback.read().unwrap().as_ref() {
                cb.on_error(e);
            }
        }
    }
}

/// Check if a message sender is the current host
fn is_from_host(from: &str, room: &Arc<RwLock<Room>>) -> bool {
    let room_guard = room.read().unwrap();
    room_guard.state()
        .map(|s| s.host_peer_id == from)
        .unwrap_or(false)
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
    seek_calibrator: &SharedSeekCalibrator,
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
            // RoomState must come from the claimed host (or we're joining and don't know yet)
            let is_joining = {
                let r = room.read().unwrap();
                matches!(&*r, Room::Joining { .. })
            };
            if is_joining || from == host_peer_id {
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
                    seek_calibrator,
                    local_peer_id,
                ).await;
            } else {
                warn!("Ignoring RoomState from non-host: {} (expected {})", from, host_peer_id);
            }
        }

        SyncMessage::ParticipantJoined(participant) => {
            // Only host can announce new participants
            if is_from_host(&from, room) {
                handle_participant_joined(participant, room, callback);
            } else {
                warn!("Ignoring ParticipantJoined from non-host: {}", from);
            }
        }

        SyncMessage::ParticipantLeft { peer_id } => {
            // Only host can announce departures
            if is_from_host(&from, room) {
                handle_participant_left(peer_id, room, callback);
            } else {
                warn!("Ignoring ParticipantLeft from non-host: {}", from);
            }
        }

        SyncMessage::TransferHost { new_host_peer_id } => {
            // Only current host can transfer
            if is_from_host(&from, room) {
                handle_transfer_host(new_host_peer_id, room, callback);
            } else {
                warn!("Ignoring TransferHost from non-host: {}", from);
            }
        }

        SyncMessage::Play { track, position_ms, .. } => {
            // Only host controls playback
            if is_from_host(&from, room) {
                handle_play(track, position_ms, room, cider, seek_calibrator).await;
            } else {
                warn!("Ignoring Play from non-host: {}", from);
            }
        }

        SyncMessage::Pause { position_ms, .. } => {
            if is_from_host(&from, room) {
                handle_pause(position_ms, room, cider).await;
            } else {
                warn!("Ignoring Pause from non-host: {}", from);
            }
        }

        SyncMessage::Seek { position_ms, .. } => {
            if is_from_host(&from, room) {
                handle_seek(position_ms, room, cider, seek_calibrator).await;
            } else {
                warn!("Ignoring Seek from non-host: {}", from);
            }
        }

        SyncMessage::TrackChange { track, position_ms, timestamp_ms } => {
            if is_from_host(&from, room) {
                handle_track_change(track, position_ms, timestamp_ms, room, callback, cider, seek_calibrator).await;
            } else {
                warn!("Ignoring TrackChange from non-host: {}", from);
            }
        }

        SyncMessage::Heartbeat { track_id: _, playback } => {
            if is_from_host(&from, room) {
                handle_heartbeat(playback, room, callback, cider, latency_tracker, seek_calibrator).await;
            } else {
                debug!("Ignoring Heartbeat from non-host: {}", from);
            }
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
            // Check if this is a new participant or updating an existing "?" entry
            let was_unknown = state.participants.get(&from)
                .map(|p| p.display_name == "?")
                .unwrap_or(false);
            let is_new = !state.participants.contains_key(&from);

            info!("Join request from {} ({}) - new: {}, was_unknown: {}",
                  display_name, from, is_new, was_unknown);

            // Add/update participant
            state.add_participant(InternalParticipant {
                peer_id: from.clone(),
                display_name: display_name.clone(),
                is_host: false,
            });

            // Notify callback
            if let Some(cb) = callback.read().unwrap().as_ref() {
                // Only fire on_participant_joined for truly new participants
                // (not for "?" → real name updates, those come via room_state_changed)
                if is_new {
                    cb.on_participant_joined(Participant {
                        peer_id: from.clone(),
                        display_name: display_name.clone(),
                        is_host: false,
                    });
                }
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
    seek_calibrator: &SharedSeekCalibrator,
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
            let seek_offset_ms = seek_calibrator.read().unwrap().offset_ms();
            let actual_position = if is_playing {
                // Add seek_offset to compensate for Cider's buffering delay
                position_ms + elapsed_since_heartbeat + seek_offset_ms
            } else {
                position_ms
            };

            info!("Seeking to adjusted position: {}ms (original: {}ms, elapsed: {}ms, offset: {}ms)",
                actual_position, position_ms, elapsed_since_heartbeat, seek_offset_ms);

            let _ = cider_client.seek_ms(actual_position).await;

            // Mark that we just seeked - next heartbeat will calibrate
            {
                let mut calibrator = seek_calibrator.write().unwrap();
                calibrator.mark_seek_performed();
            }
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
    seek_calibrator: &SharedSeekCalibrator,
) {
    // Non-host: sync to host's playback
    let should_sync = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| !s.is_host()).unwrap_or(false)
    };

    if should_sync {
        let cider_client = cider.read().unwrap().clone();
        let song_id = track.song_id.clone();
        let seek_offset_ms = seek_calibrator.read().unwrap().offset_ms();
        // Play the same track at the same position + offset to compensate for buffer delay
        let _ = cider_client.play_item("songs", &song_id).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = cider_client.seek_ms(position_ms + seek_offset_ms).await;
        let _ = cider_client.play().await;

        // Mark that we just seeked - next heartbeat will calibrate
        {
            let mut calibrator = seek_calibrator.write().unwrap();
            calibrator.mark_seek_performed();
        }
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
    seek_calibrator: &SharedSeekCalibrator,
) {
    let should_sync = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| !s.is_host()).unwrap_or(false)
    };

    if should_sync {
        let cider_client = cider.read().unwrap().clone();
        let seek_offset_ms = seek_calibrator.read().unwrap().offset_ms();
        let _ = cider_client.seek_ms(position_ms + seek_offset_ms).await;

        // Mark that we just seeked - next heartbeat will calibrate
        {
            let mut calibrator = seek_calibrator.write().unwrap();
            calibrator.mark_seek_performed();
        }
    }
}

async fn handle_track_change(
    track: crate::sync::TrackInfo,
    position_ms: u64,
    timestamp_ms: u64,
    room: &Arc<RwLock<Room>>,
    callback: &Arc<RwLock<Option<Arc<dyn SessionCallback>>>>,
    cider: &Arc<RwLock<CiderClient>>,
    seek_calibrator: &SharedSeekCalibrator,
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

        // Calculate actual position accounting for elapsed time + seek offset
        let now = super::types::current_time_ms();
        let elapsed = now.saturating_sub(timestamp_ms);
        let seek_offset_ms = seek_calibrator.read().unwrap().offset_ms();
        let actual_position = position_ms + elapsed + seek_offset_ms;

        info!("TrackChange: seeking to {}ms (original: {}ms, elapsed: {}ms, offset: {}ms)",
            actual_position, position_ms, elapsed, seek_offset_ms);

        let _ = cider_client.seek_ms(actual_position).await;

        // Mark that we just seeked - next heartbeat will calibrate
        {
            let mut calibrator = seek_calibrator.write().unwrap();
            calibrator.mark_seek_performed();
        }
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
    seek_calibrator: &SharedSeekCalibrator,
) {
    // Check if we're a listener and need to sync
    let should_sync = {
        let room_guard = room.read().unwrap();
        room_guard.state().map(|s| !s.is_host()).unwrap_or(false)
    };

    if should_sync {
        // Get estimated one-way latency to host and seek offset
        let latency_ms = latency_tracker.read().unwrap().host_latency_ms();
        let seek_offset_ms = seek_calibrator.read().unwrap().offset_ms();

        // Get current Cider playback state first
        let cider_client = cider.read().unwrap().clone();

        // Check current position from now_playing
        if let Ok(Some(np)) = cider_client.now_playing().await {
            // Calculate expected position NOW (after async call completes)
            // This gives more accurate comparison since current_position is also "now"
            let now = super::types::current_time_ms();
            let elapsed_since_heartbeat = now.saturating_sub(playback.timestamp_ms);

            // Expected position for COMPARISON (where host actually is + network latency)
            // Does NOT include seek_offset - that's only for when we actually seek
            let expected_position = if playback.is_playing {
                playback.position_ms + elapsed_since_heartbeat + latency_ms
            } else {
                playback.position_ms
            };
            let current_position = np.current_position_ms();

            // Check if we're drifted too far from expected position
            let drift_signed = current_position as i64 - expected_position as i64;
            let drift = drift_signed.unsigned_abs();

            // Log sync accuracy for diagnostics (positive = ahead, negative = behind)
            debug!(
                "Sync: drift {:+}ms (expected: {}ms, actual: {}ms, latency: {}ms, seek_offset: {}ms, elapsed: {}ms)",
                drift_signed, expected_position, current_position, latency_ms, seek_offset_ms, elapsed_since_heartbeat
            );

            // Get calibration state for debug display (before we potentially update it)
            let (calibration_pending, next_calibration_sample, sample_history) = {
                let calibrator = seek_calibrator.read().unwrap();
                let pending = calibrator.is_awaiting_measurement();
                let sample = if pending {
                    calibrator.preview_calibration(drift_signed)
                } else {
                    None
                };
                let history: Vec<CalibrationSample> = calibrator
                    .sample_history()
                    .iter()
                    .map(CalibrationSample::from)
                    .collect();
                (pending, sample, history)
            };

            // Report sync status to UI for debug display
            if let Some(cb) = callback.read().unwrap().as_ref() {
                cb.on_sync_status(SyncStatus {
                    drift_ms: drift_signed,
                    latency_ms,
                    elapsed_ms: elapsed_since_heartbeat,
                    seek_offset_ms,
                    calibration_pending,
                    next_calibration_sample,
                    sample_history,
                });
            }

            // Try to measure the result of a previous seek operation (only updates if we were awaiting)
            {
                let mut calibrator = seek_calibrator.write().unwrap();
                calibrator.measure_if_pending(drift_signed);
            }

            if drift > DRIFT_THRESHOLD_MS {
                // When seeking, ADD seek_offset to compensate for Cider's buffering delay
                let seek_target = expected_position + seek_offset_ms;
                info!(
                    "Heartbeat: position drift {}ms exceeds threshold, re-syncing (target: {}ms, current: {}ms, offset: {}ms)",
                    drift, seek_target, current_position, seek_offset_ms
                );
                let _ = cider_client.seek_ms(seek_target).await;

                // Mark that we just seeked - next heartbeat will measure how accurate it was
                {
                    let mut calibrator = seek_calibrator.write().unwrap();
                    calibrator.mark_seek_performed();
                }
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
