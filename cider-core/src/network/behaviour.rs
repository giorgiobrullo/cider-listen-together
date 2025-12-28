//! libp2p Network Behaviour
//!
//! Implements the P2P networking layer using libp2p with:
//! - mDNS for local network discovery
//! - TCP + QUIC transports for connectivity
//! - Relay client for NAT traversal (internet connectivity)
//! - DCUtR for hole punching (direct connections through NAT)

use futures::StreamExt;
use libp2p::{
    dcutr, gossipsub, identify, identity, mdns, noise, relay, swarm::NetworkBehaviour,
    swarm::SwarmEvent, tcp, yamux, Multiaddr, PeerId, Swarm,
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
    // mars.i.ipfs.io - TCP
    "/ip4/104.131.131.82/tcp/4001/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
    // mars.i.ipfs.io - QUIC
    "/ip4/104.131.131.82/udp/4001/quic-v1/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
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

/// Combined network behaviour with mDNS + Relay for internet connectivity
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
    /// Peers subscribed to our room topic
    room_peers: HashSet<PeerId>,
    /// Connected relay servers
    connected_relays: HashSet<PeerId>,
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
            room_peers: HashSet::new(),
            connected_relays: HashSet::new(),
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

                Ok(CiderBehaviour {
                    relay_client,
                    dcutr,
                    mdns,
                    identify,
                    gossipsub,
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
                            }
                        }
                        NetworkCommand::JoinRoom { room_code } => {
                            if let Err(e) = self.join_room(&mut swarm, &room_code) {
                                let _ = event_tx.send(NetworkEvent::Error(e.to_string()));
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

            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                debug!("Connection established with {}", peer_id);
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
                    debug!("Failed to connect to {}: {}", peer, error);
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

        info!("Created and subscribed to room: {}", room_code);
        self.room_topic = Some(topic);
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

        info!("Joined room: {}", room_code);
        self.room_topic = Some(topic);
        self.room_peers.clear();

        Ok(())
    }

    /// Leave the current room
    fn leave_room(&mut self, swarm: &mut Swarm<CiderBehaviour>) -> Result<(), NetworkError> {
        if let Some(topic) = self.room_topic.take() {
            let _ = swarm.behaviour_mut().gossipsub.unsubscribe(&topic);
            info!("Left room");
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
