//! libp2p Network Behaviour
//!
//! Implements the P2P networking layer using libp2p with:
//! - mDNS for local network discovery
//! - TCP + QUIC transports for connectivity
//! - Relay client for NAT traversal (internet connectivity)
//! - DCUtR for hole punching (direct connections through NAT)

use futures::StreamExt;
use libp2p::{
    dcutr, gossipsub, identify, identity, kad, mdns, noise, relay, swarm::NetworkBehaviour,
    swarm::SwarmEvent, tcp, yamux, Multiaddr, PeerId, StreamProtocol, Swarm,
};
use std::collections::HashSet;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::sync::SyncMessage;

/// Public IPFS bootstrap nodes with direct TCP/QUIC addresses
/// Using direct IP addresses to avoid DNS resolution issues with /dnsaddr
const BOOTSTRAP_NODES: &[&str] = &[
    // mars.i.ipfs.io
    "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
    "/ip4/104.131.131.82/udp/4001/quic-v1/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
    // saturn.i.ipfs.io
    "/ip4/178.128.122.218/tcp/4001/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "/ip4/178.128.122.218/udp/4001/quic-v1/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    // pluto.i.ipfs.io
    "/ip4/139.178.68.217/tcp/4001/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "/ip4/139.178.68.217/udp/4001/quic-v1/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    // neptune.i.ipfs.io
    "/ip4/128.199.219.111/tcp/4001/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "/ip4/128.199.219.111/udp/4001/quic-v1/p2p/QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
];

/// Network-related errors
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Failed to create transport: {0}")]
    Transport(String),

    #[error("Failed to connect to peer: {0}")]
    Connection(String),

    #[error("Room not found: {0}")]
    RoomNotFound(String),

    #[error("Already in a room")]
    AlreadyInRoom,

    #[error("Not in a room")]
    NotInRoom,

    #[error("libp2p error: {0}")]
    Libp2p(String),

    #[error("Join timeout")]
    JoinTimeout,
}

/// Combined network behaviour with mDNS + Relay + DHT for internet connectivity
#[derive(NetworkBehaviour)]
pub struct CiderBehaviour {
    /// Relay client for NAT traversal
    relay_client: relay::client::Behaviour,
    /// DCUtR for hole punching (direct connections through relay)
    dcutr: dcutr::Behaviour,
    /// mDNS for local network discovery
    mdns: mdns::tokio::Behaviour,
    /// Peer identification
    identify: identify::Behaviour,
    /// Pub/sub for room messages
    gossipsub: gossipsub::Behaviour,
    /// Kademlia DHT for peer discovery over internet
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

/// Events emitted by the network manager
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Network is ready (listening)
    Ready { peer_id: String },
    /// Received a sync message from a peer
    Message { from: String, message: SyncMessage },
    /// A peer subscribed to our room topic
    PeerSubscribed { peer_id: String },
    /// A peer unsubscribed from our room topic
    PeerUnsubscribed { peer_id: String },
    /// Current listening addresses (sent after room creation/join)
    ListeningAddresses { addresses: Vec<String> },
    /// Error occurred
    Error(String),
}

/// Commands sent to the network manager
#[derive(Debug)]
pub enum NetworkCommand {
    /// Create a room with the given code
    CreateRoom { room_code: String },
    /// Join a room with the given code
    JoinRoom { room_code: String },
    /// Leave the current room
    LeaveRoom,
    /// Broadcast a message to the room
    Broadcast { message: SyncMessage },
    /// Dial a peer directly by multiaddr (for manual connection)
    DialPeer { multiaddr: String },
    /// Shutdown the network
    Shutdown,
}

/// Handle to communicate with the running network
#[derive(Clone)]
pub struct NetworkHandle {
    command_tx: mpsc::UnboundedSender<NetworkCommand>,
    pub local_peer_id: String,
}

impl NetworkHandle {
    pub fn create_room(&self, room_code: &str) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::CreateRoom {
                room_code: room_code.to_string(),
            })
            .map_err(|_| NetworkError::Libp2p("Network task closed".to_string()))
    }

    pub fn join_room(&self, room_code: &str) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::JoinRoom {
                room_code: room_code.to_string(),
            })
            .map_err(|_| NetworkError::Libp2p("Network task closed".to_string()))
    }

    pub fn leave_room(&self) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::LeaveRoom)
            .map_err(|_| NetworkError::Libp2p("Network task closed".to_string()))
    }

    pub fn broadcast(&self, message: SyncMessage) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::Broadcast { message })
            .map_err(|_| NetworkError::Libp2p("Network task closed".to_string()))
    }

    pub fn shutdown(&self) {
        let _ = self.command_tx.send(NetworkCommand::Shutdown);
    }

    pub fn dial_peer(&self, multiaddr: &str) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::DialPeer {
                multiaddr: multiaddr.to_string(),
            })
            .map_err(|_| NetworkError::Libp2p("Network task closed".to_string()))
    }
}

/// Manages P2P networking - runs in a background task
pub struct NetworkManager {
    /// Our local peer ID
    local_peer_id: PeerId,
    /// Our keypair
    keypair: identity::Keypair,
    /// Discovered peers (via mDNS or relay)
    discovered_peers: HashSet<PeerId>,
    /// Current room topic (if in a room)
    room_topic: Option<gossipsub::IdentTopic>,
    /// Current room code (for DHT cleanup)
    room_code: Option<String>,
    /// Peers subscribed to our room topic
    room_peers: HashSet<PeerId>,
    /// Connected relay servers
    connected_relays: HashSet<PeerId>,
    /// Our listening addresses (for signaling)
    listening_addresses: Vec<String>,
}

impl NetworkManager {
    /// Create a new network manager
    pub fn new() -> Result<Self, NetworkError> {
        let keypair = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());

        info!("Local peer ID: {}", local_peer_id);

        Ok(Self {
            local_peer_id,
            keypair,
            discovered_peers: HashSet::new(),
            room_topic: None,
            room_code: None,
            room_peers: HashSet::new(),
            connected_relays: HashSet::new(),
            listening_addresses: Vec::new(),
        })
    }

    /// Get our local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }

    /// Get our local peer ID as string
    pub fn local_peer_id_string(&self) -> String {
        self.local_peer_id.to_string()
    }

    /// Start the network and return a handle for communication
    pub fn start(
        self,
    ) -> Result<(NetworkHandle, mpsc::UnboundedReceiver<NetworkEvent>), NetworkError> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let local_peer_id = self.local_peer_id.to_string();

        let handle = NetworkHandle {
            command_tx,
            local_peer_id: local_peer_id.clone(),
        };

        // Spawn the network task
        tokio::spawn(async move {
            if let Err(e) = self.run(event_tx, command_rx).await {
                warn!("Network task error: {}", e);
            }
        });

        Ok((handle, event_rx))
    }

    /// Create the libp2p swarm with relay support
    ///
    /// Transport chain: TCP (for relay) -> QUIC (for direct) -> DNS -> Relay Client
    fn create_swarm(&self) -> Result<Swarm<CiderBehaviour>, NetworkError> {
        let swarm = libp2p::SwarmBuilder::with_existing_identity(self.keypair.clone())
            .with_tokio()
            // TCP first - needed for relay protocol (uses noise+yamux)
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| NetworkError::Transport(e.to_string()))?
            // QUIC for direct connections (has built-in encryption/mux)
            .with_quic()
            // DNS resolution for bootstrap nodes
            .with_dns()
            .map_err(|e| NetworkError::Transport(e.to_string()))?
            // Relay client for NAT traversal (runs over TCP's noise+yamux)
            .with_relay_client(noise::Config::new, yamux::Config::default)
            .map_err(|e| NetworkError::Transport(e.to_string()))?
            .with_behaviour(|keypair, relay_client| {
                // mDNS for local discovery
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    keypair.public().to_peer_id(),
                )
                .map_err(|e| e.to_string())?;

                // DCUtR for hole punching
                let dcutr = dcutr::Behaviour::new(keypair.public().to_peer_id());

                // Gossipsub config - tuned for small networks
                // Must satisfy: mesh_outbound_min <= mesh_n_low <= mesh_n <= mesh_n_high
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(1))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .mesh_outbound_min(0) // Allow functioning with no outbound peers
                    .mesh_n_low(1)
                    .mesh_n(3)
                    .mesh_n_high(6)
                    .gossip_lazy(3)
                    .build()
                    .map_err(|e| e.to_string())?;

                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                    gossipsub_config,
                )
                .map_err(|e| e.to_string())?;

                // Identify config
                let identify = identify::Behaviour::new(identify::Config::new(
                    "/cider-together/1.0.0".into(),
                    keypair.public(),
                ));

                // Kademlia DHT for peer discovery
                // Use IPFS protocol to leverage the public IPFS DHT network
                let local_peer_id = keypair.public().to_peer_id();
                let store = kad::store::MemoryStore::new(local_peer_id);
                let mut kademlia_config = kad::Config::new(StreamProtocol::new("/ipfs/kad/1.0.0"));
                kademlia_config.set_query_timeout(Duration::from_secs(60));
                // Allow Kademlia to auto-detect mode based on whether we're publicly reachable
                // (Server if reachable, Client if behind NAT)
                kademlia_config.set_kbucket_inserts(kad::BucketInserts::OnConnected);
                let mut kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);
                // Don't force server mode - let libp2p auto-detect based on connectivity
                // kademlia.set_mode(None) is the default and enables auto-mode

                // Add bootstrap nodes to Kademlia routing table
                for addr_str in BOOTSTRAP_NODES {
                    if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                        // Extract peer ID from the address
                        if let Some(libp2p::multiaddr::Protocol::P2p(peer_id)) = addr.iter().last() {
                            kademlia.add_address(&peer_id, addr.clone());
                        }
                    }
                }

                Ok(CiderBehaviour {
                    relay_client,
                    dcutr,
                    mdns,
                    identify,
                    gossipsub,
                    kademlia,
                })
            })
            .map_err(|e| NetworkError::Transport(e.to_string()))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        Ok(swarm)
    }

    /// Connect to bootstrap relay nodes for internet connectivity
    fn connect_to_bootstrap_nodes(&self, swarm: &mut Swarm<CiderBehaviour>) {
        for addr_str in BOOTSTRAP_NODES {
            if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                info!("Connecting to bootstrap node: {}", addr);
                if let Err(e) = swarm.dial(addr.clone()) {
                    debug!("Failed to dial bootstrap node {}: {}", addr, e);
                }
            }
        }
    }

    /// Run the network event loop
    async fn run(
        mut self,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        mut command_rx: mpsc::UnboundedReceiver<NetworkCommand>,
    ) -> Result<(), NetworkError> {
        let mut swarm = self.create_swarm()?;

        // Listen on TCP (for relay connections)
        match swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()) {
            Ok(id) => info!("TCP listener started: {:?}", id),
            Err(e) => warn!("Failed to listen on TCP: {:?}", e),
        }

        // Listen on QUIC (for direct connections)
        match swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap()) {
            Ok(id) => info!("QUIC listener started: {:?}", id),
            Err(e) => warn!("Failed to listen on QUIC: {:?}", e),
        }

        // Connect to bootstrap nodes for internet connectivity
        self.connect_to_bootstrap_nodes(&mut swarm);

        // Bootstrap the Kademlia DHT
        if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
            warn!("Failed to bootstrap Kademlia DHT: {:?}", e);
        } else {
            info!("Kademlia DHT bootstrap started");
        }

        // Notify ready
        let _ = event_tx.send(NetworkEvent::Ready {
            peer_id: self.local_peer_id.to_string(),
        });

        loop {
            tokio::select! {
                // Handle swarm events
                event = swarm.select_next_some() => {
                    self.handle_swarm_event(&mut swarm, event, &event_tx);
                }
                // Handle commands
                Some(cmd) = command_rx.recv() => {
                    match cmd {
                        NetworkCommand::CreateRoom { room_code } => {
                            if let Err(e) = self.create_room(&mut swarm, &room_code) {
                                let _ = event_tx.send(NetworkEvent::Error(e.to_string()));
                            } else {
                                // Send listening addresses for signaling
                                let _ = event_tx.send(NetworkEvent::ListeningAddresses {
                                    addresses: self.listening_addresses.clone(),
                                });
                            }
                        }
                        NetworkCommand::JoinRoom { room_code } => {
                            if let Err(e) = self.join_room(&mut swarm, &room_code) {
                                let _ = event_tx.send(NetworkEvent::Error(e.to_string()));
                            } else {
                                // Send listening addresses for signaling
                                let _ = event_tx.send(NetworkEvent::ListeningAddresses {
                                    addresses: self.listening_addresses.clone(),
                                });
                            }
                        }
                        NetworkCommand::LeaveRoom => {
                            let _ = self.leave_room(&mut swarm);
                        }
                        NetworkCommand::Broadcast { message } => {
                            if let Err(e) = self.broadcast(&mut swarm, &message) {
                                debug!("Broadcast error (may be no peers yet): {}", e);
                            }
                        }
                        NetworkCommand::DialPeer { multiaddr } => {
                            match multiaddr.parse::<Multiaddr>() {
                                Ok(addr) => {
                                    info!("Dialing peer at {}", addr);
                                    if let Err(e) = swarm.dial(addr) {
                                        warn!("Failed to dial peer: {}", e);
                                    }
                                }
                                Err(e) => {
                                    warn!("Invalid multiaddr {}: {}", multiaddr, e);
                                }
                            }
                        }
                        NetworkCommand::Shutdown => {
                            info!("Network shutting down");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_swarm_event(
        &mut self,
        swarm: &mut Swarm<CiderBehaviour>,
        event: SwarmEvent<CiderBehaviourEvent>,
        event_tx: &mpsc::UnboundedSender<NetworkEvent>,
    ) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}", address);
                // Track address with our peer ID appended for dial-ability
                let full_addr = format!("{}/p2p/{}", address, self.local_peer_id);
                self.listening_addresses.push(full_addr.clone());

                // If we're in a room, notify about new address for signaling
                // This is important for relay addresses which are discovered after room creation
                if self.room_topic.is_some() {
                    info!("New address discovered while in room: {}", full_addr);
                    let _ = event_tx.send(NetworkEvent::ListeningAddresses {
                        addresses: self.listening_addresses.clone(),
                    });
                }
            }

            // mDNS discovered peers (local network)
            SwarmEvent::Behaviour(CiderBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
                for (peer_id, addr) in peers {
                    if peer_id != self.local_peer_id {
                        info!("mDNS discovered peer: {} at {}", peer_id, addr);
                        self.discovered_peers.insert(peer_id);

                        // Add the peer and dial them
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        if swarm.dial(addr.clone()).is_ok() {
                            debug!("Dialing discovered peer {}", peer_id);
                        }
                    }
                }
            }

            SwarmEvent::Behaviour(CiderBehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
                for (peer_id, _) in peers {
                    debug!("mDNS peer expired: {}", peer_id);
                    self.discovered_peers.remove(&peer_id);
                }
            }

            // Relay events
            SwarmEvent::Behaviour(CiderBehaviourEvent::RelayClient(
                relay::client::Event::ReservationReqAccepted { relay_peer_id, .. },
            )) => {
                info!("Relay reservation accepted by {}", relay_peer_id);
                self.connected_relays.insert(relay_peer_id);
            }

            // DCUtR events (hole punching)
            SwarmEvent::Behaviour(CiderBehaviourEvent::Dcutr(dcutr::Event {
                remote_peer_id,
                result,
            })) => {
                match result {
                    Ok(_) => info!("DCUtR hole punch succeeded with {}", remote_peer_id),
                    Err(e) => debug!("DCUtR hole punch failed with {}: {:?}", remote_peer_id, e),
                }
            }

            // Gossipsub messages
            SwarmEvent::Behaviour(CiderBehaviourEvent::Gossipsub(
                gossipsub::Event::Message {
                    propagation_source,
                    message,
                    ..
                },
            )) => {
                if let Ok(sync_msg) = serde_json::from_slice::<SyncMessage>(&message.data) {
                    debug!("Received message from {}: {:?}", propagation_source, sync_msg);
                    let _ = event_tx.send(NetworkEvent::Message {
                        from: propagation_source.to_string(),
                        message: sync_msg,
                    });
                }
            }

            // Peer subscribed to topic
            SwarmEvent::Behaviour(CiderBehaviourEvent::Gossipsub(
                gossipsub::Event::Subscribed { peer_id, topic },
            )) => {
                if let Some(our_topic) = &self.room_topic {
                    if topic == our_topic.hash() {
                        info!("Peer {} subscribed to room", peer_id);
                        self.room_peers.insert(peer_id);
                        let _ = event_tx.send(NetworkEvent::PeerSubscribed {
                            peer_id: peer_id.to_string(),
                        });
                    }
                }
            }

            // Peer unsubscribed from topic
            SwarmEvent::Behaviour(CiderBehaviourEvent::Gossipsub(
                gossipsub::Event::Unsubscribed { peer_id, topic },
            )) => {
                if let Some(our_topic) = &self.room_topic {
                    if topic == our_topic.hash() {
                        info!("Peer {} unsubscribed from room", peer_id);
                        self.room_peers.remove(&peer_id);
                        let _ = event_tx.send(NetworkEvent::PeerUnsubscribed {
                            peer_id: peer_id.to_string(),
                        });
                    }
                }
            }

            SwarmEvent::Behaviour(CiderBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => {
                debug!(
                    "Identified peer {} running {}",
                    peer_id, info.protocol_version
                );

                // If this is a relay server, listen through it for incoming connections
                for addr in info.listen_addrs {
                    // Check if this peer supports relay
                    if info.protocols.iter().any(|p| p.as_ref().contains("relay")) {
                        let relay_addr = addr
                            .clone()
                            .with(libp2p::multiaddr::Protocol::P2pCircuit);
                        info!("Listening through relay: {}", relay_addr);
                        let _ = swarm.listen_on(relay_addr);
                    }
                }
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Connection established with {} via {:?}", peer_id, endpoint);
                // Add to gossipsub for mesh
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
            }

            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                debug!("Connection closed with {}", peer_id);
                self.room_peers.remove(&peer_id);
                self.connected_relays.remove(&peer_id);
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer) = peer_id {
                    warn!("Failed to connect to {}: {}", peer, error);
                } else {
                    warn!("Outgoing connection error: {}", error);
                }
            }

            // Kademlia DHT events
            SwarmEvent::Behaviour(CiderBehaviourEvent::Kademlia(event)) => {
                match event {
                    kad::Event::RoutingUpdated { peer, is_new_peer, .. } => {
                        info!("Kademlia routing updated: peer={}, new={}", peer, is_new_peer);
                    }
                    kad::Event::ModeChanged { new_mode } => {
                        info!("Kademlia mode changed to: {:?}", new_mode);
                    }
                    kad::Event::OutboundQueryProgressed { id, result, stats, step } => {
                        debug!("Kademlia query {:?} progressed: step={:?}, stats={:?}", id, step, stats);
                        match result {
                            kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { peer, num_remaining })) => {
                                info!("Kademlia bootstrap progress: peer={}, remaining={}", peer, num_remaining);
                                if num_remaining == 0 {
                                    info!("Kademlia bootstrap complete!");
                                }
                            }
                            kad::QueryResult::Bootstrap(Err(e)) => {
                                warn!("Kademlia bootstrap error: {:?}", e);
                            }
                            kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders { providers, .. })) => {
                                info!("DHT found {} providers for room", providers.len());
                                for provider in providers {
                                    if provider != self.local_peer_id {
                                        debug!("Found room provider: {}", provider);
                                        // Add to gossipsub and try to connect
                                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&provider);
                                        // Dial the peer through known addresses
                                        if let Err(e) = swarm.dial(provider) {
                                            debug!("Failed to dial provider {}: {}", provider, e);
                                        }
                                    }
                                }
                            }
                            kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. })) => {
                                debug!("DHT provider search finished");
                            }
                            kad::QueryResult::GetProviders(Err(e)) => {
                                debug!("DHT get providers error: {:?}", e);
                            }
                            kad::QueryResult::StartProviding(Ok(kad::AddProviderOk { key })) => {
                                info!("DHT: Now providing room {:?}", String::from_utf8_lossy(key.as_ref()));
                            }
                            kad::QueryResult::StartProviding(Err(e)) => {
                                warn!("DHT start providing error: {:?}", e);
                            }
                            kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                                info!("DHT GetClosestPeers: found {} peers", ok.peers.len());
                            }
                            kad::QueryResult::GetClosestPeers(Err(e)) => {
                                warn!("DHT GetClosestPeers error: {:?}", e);
                            }
                            other => {
                                debug!("DHT query result: {:?}", other);
                            }
                        }
                    }
                    kad::Event::InboundRequest { request } => {
                        debug!("Kademlia inbound request: {:?}", request);
                    }
                    kad::Event::UnroutablePeer { peer } => {
                        debug!("Kademlia unroutable peer: {}", peer);
                    }
                    kad::Event::PendingRoutablePeer { peer, address } => {
                        debug!("Kademlia pending routable peer: {} at {}", peer, address);
                    }
                    kad::Event::RoutablePeer { peer, address } => {
                        info!("Kademlia routable peer: {} at {}", peer, address);
                    }
                }
            }

            _ => {}
        }
    }

    /// Create a room and subscribe to its topic
    fn create_room(
        &mut self,
        swarm: &mut Swarm<CiderBehaviour>,
        room_code: &str,
    ) -> Result<(), NetworkError> {
        if self.room_topic.is_some() {
            return Err(NetworkError::AlreadyInRoom);
        }

        let topic = gossipsub::IdentTopic::new(format!("cider-room-{}", room_code));

        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .map_err(|e| NetworkError::Libp2p(e.to_string()))?;

        // Advertise this room in the DHT so others can find us
        let room_key = kad::RecordKey::new(&format!("cider-room-{}", room_code));
        if let Err(e) = swarm.behaviour_mut().kademlia.start_providing(room_key.clone()) {
            warn!("Failed to start providing room in DHT: {:?}", e);
        } else {
            info!("DHT: Advertising room {} to the network", room_code);
        }

        info!("Created and subscribed to room: {}", room_code);
        self.room_topic = Some(topic);
        self.room_code = Some(room_code.to_string());
        self.room_peers.clear();

        Ok(())
    }

    /// Join a room by subscribing to its topic
    fn join_room(
        &mut self,
        swarm: &mut Swarm<CiderBehaviour>,
        room_code: &str,
    ) -> Result<(), NetworkError> {
        if self.room_topic.is_some() {
            return Err(NetworkError::AlreadyInRoom);
        }

        let topic = gossipsub::IdentTopic::new(format!("cider-room-{}", room_code));

        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topic)
            .map_err(|e| NetworkError::Libp2p(e.to_string()))?;

        // Search DHT for peers in this room
        let room_key = kad::RecordKey::new(&format!("cider-room-{}", room_code));
        swarm.behaviour_mut().kademlia.get_providers(room_key.clone());
        info!("DHT: Searching for peers in room {}", room_code);

        // Also advertise ourselves so others can find us
        if let Err(e) = swarm.behaviour_mut().kademlia.start_providing(room_key) {
            warn!("Failed to start providing room in DHT: {:?}", e);
        }

        info!("Joined room: {}", room_code);
        self.room_topic = Some(topic);
        self.room_code = Some(room_code.to_string());
        self.room_peers.clear();

        Ok(())
    }

    /// Leave the current room
    fn leave_room(&mut self, swarm: &mut Swarm<CiderBehaviour>) -> Result<(), NetworkError> {
        if let Some(topic) = self.room_topic.take() {
            let _ = swarm.behaviour_mut().gossipsub.unsubscribe(&topic);
            info!("Left room");
        }

        // Stop providing in DHT
        if let Some(code) = self.room_code.take() {
            let room_key = kad::RecordKey::new(&format!("cider-room-{}", code));
            swarm.behaviour_mut().kademlia.stop_providing(&room_key);
            info!("DHT: Stopped advertising room {}", code);
        }

        self.room_peers.clear();
        Ok(())
    }

    /// Broadcast a message to the room
    fn broadcast(
        &self,
        swarm: &mut Swarm<CiderBehaviour>,
        message: &SyncMessage,
    ) -> Result<(), NetworkError> {
        let topic = self.room_topic.as_ref().ok_or(NetworkError::NotInRoom)?;

        let data =
            serde_json::to_vec(message).map_err(|e| NetworkError::Libp2p(e.to_string()))?;

        swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.clone(), data)
            .map_err(|e| NetworkError::Libp2p(e.to_string()))?;

        Ok(())
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new().expect("Failed to create NetworkManager")
    }
}
