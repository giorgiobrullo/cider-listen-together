//! Network handling for the relay server

use crate::metrics::{LogLevel, Metrics, ServerStatus, truncate_peer_id};
use futures::StreamExt;
use libp2p::{
    identify, identity, kad, noise, ping, relay, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp,
    yamux, Multiaddr, PeerId, StreamProtocol, Swarm,
};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Default keypair file name
const KEYPAIR_FILE: &str = "keypair.bin";

/// How long to wait for a peer to identify before disconnecting
const IDENTIFY_TIMEOUT_SECS: u64 = 30;

/// Required protocol prefix for Cider clients
const CIDER_PROTOCOL_PREFIX: &str = "cider";

/// Combined behaviour for the relay server
#[derive(NetworkBehaviour)]
pub struct RelayServerBehaviour {
    pub ping: ping::Behaviour,
    pub relay: relay::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

/// Events sent from network to dashboard
#[derive(Debug)]
#[allow(dead_code)]
pub enum NetworkEvent {
    Ready { peer_id: String },
    PublicIp(Option<String>),
    PortCheck(bool),
}

/// Get the path to the keypair file
fn get_keypair_path() -> PathBuf {
    // Check for custom path via env var
    if let Ok(path) = std::env::var("KEYPAIR_PATH") {
        return PathBuf::from(path);
    }

    // Default: same directory as executable, or current dir
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(KEYPAIR_FILE)
}

/// Load existing keypair or generate a new one
fn load_or_create_keypair() -> Result<identity::Keypair, Box<dyn Error>> {
    let path = get_keypair_path();

    if path.exists() {
        // Load existing keypair
        let bytes = fs::read(&path)?;
        let keypair = identity::Keypair::from_protobuf_encoding(&bytes)?;
        info!("Loaded existing keypair from {}", path.display());
        Ok(keypair)
    } else {
        // Generate new keypair and save it
        let keypair = identity::Keypair::generate_ed25519();
        let bytes = keypair.to_protobuf_encoding()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, bytes)?;
        info!("Generated new keypair, saved to {}", path.display());
        Ok(keypair)
    }
}

/// Create and configure the swarm
pub fn create_swarm(keypair: &identity::Keypair) -> Result<Swarm<RelayServerBehaviour>, Box<dyn Error>> {
    let local_peer_id = keypair.public().to_peer_id();

    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|keypair| {
            // Ping for keep-alive (every 15 seconds)
            let ping = ping::Behaviour::new(
                ping::Config::new()
                    .with_interval(Duration::from_secs(15))
                    .with_timeout(Duration::from_secs(20)),
            );

            let relay_config = relay::Config::default();
            let relay = relay::Behaviour::new(keypair.public().to_peer_id(), relay_config);

            let identify = identify::Behaviour::new(identify::Config::new(
                "/cider-relay/1.0.0".into(),
                keypair.public(),
            ));

            let store = kad::store::MemoryStore::new(local_peer_id);
            let mut kademlia_config = kad::Config::new(StreamProtocol::new("/ipfs/kad/1.0.0"));
            kademlia_config.set_query_timeout(Duration::from_secs(60));
            let kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);

            Ok(RelayServerBehaviour {
                ping,
                relay,
                identify,
                kademlia,
            })
        })?
        // Longer timeout to keep client connections alive while waiting for peers
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(300)))
        .build();

    Ok(swarm)
}

/// Run the network with dashboard integration
pub async fn run_with_dashboard(
    metrics: Arc<RwLock<Metrics>>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
) -> Result<(), Box<dyn Error>> {
    let keypair = load_or_create_keypair()?;
    let local_peer_id = PeerId::from(keypair.public());

    info!("Cider Relay Server starting...");
    info!("Peer ID: {}", local_peer_id);

    // Update metrics with peer ID
    {
        let mut m = metrics.write();
        m.peer_id = Some(local_peer_id.to_string());
        m.log(LogLevel::Info, format!("Peer ID: {}", local_peer_id));
    }

    let mut swarm = create_swarm(&keypair)?;

    // Get ports from env
    let tcp_port = std::env::var("TCP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4001u16);
    let quic_port = std::env::var("QUIC_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4001u16);

    {
        let mut m = metrics.write();
        m.tcp_port = tcp_port;
        m.quic_port = quic_port;
    }

    // Listen on IPv4
    let tcp_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", tcp_port).parse()?;
    let quic_addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", quic_port).parse()?;
    swarm.listen_on(tcp_addr)?;
    swarm.listen_on(quic_addr)?;

    // Listen on IPv6 (if available)
    let tcp6_addr: Multiaddr = format!("/ip6/::/tcp/{}", tcp_port).parse()?;
    let quic6_addr: Multiaddr = format!("/ip6/::/udp/{}/quic-v1", quic_port).parse()?;
    let _ = swarm.listen_on(tcp6_addr); // Ignore error if IPv6 not available
    let _ = swarm.listen_on(quic6_addr);

    // Notify ready
    let _ = event_tx.send(NetworkEvent::Ready {
        peer_id: local_peer_id.to_string(),
    });

    {
        let mut m = metrics.write();
        m.status = ServerStatus::Running;
        m.log(LogLevel::Info, format!("Listening on TCP:{} QUIC:{}", tcp_port, quic_port));
    }

    // Detect public IP and add external addresses BEFORE starting event loop
    // This ensures clients get the correct addresses when they identify us
    info!("Detecting public IP address...");
    if let Some(public_ip) = detect_public_ip().await {
        info!("Public IP detected: {}", public_ip);

        // Add external addresses so clients can see our public IP via identify
        let tcp_external: Multiaddr = format!("/ip4/{}/tcp/{}", public_ip, tcp_port).parse()
            .expect("valid multiaddr");
        let quic_external: Multiaddr = format!("/ip4/{}/udp/{}/quic-v1", public_ip, quic_port).parse()
            .expect("valid multiaddr");

        info!("Adding external TCP address: {}", tcp_external);
        swarm.add_external_address(tcp_external);
        info!("Adding external QUIC address: {}", quic_external);
        swarm.add_external_address(quic_external);

        {
            let mut m = metrics.write();
            m.public_ip = Some(public_ip.clone());
            m.log(LogLevel::Info, format!("Public IP: {}", public_ip));
        }
        let _ = event_tx.send(NetworkEvent::PublicIp(Some(public_ip.clone())));

        // Run port check in background (non-blocking)
        let metrics_clone = Arc::clone(&metrics);
        let event_tx_clone = event_tx.clone();
        let ip_clone = public_ip.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let reachable = check_port_reachable(&ip_clone, tcp_port).await;
            let _ = event_tx_clone.send(NetworkEvent::PortCheck(reachable));

            let mut m = metrics_clone.write();
            m.tcp_reachable = Some(reachable);
            if reachable {
                info!("TCP port {} is reachable from internet", tcp_port);
                m.log(LogLevel::Info, format!("TCP port {} is reachable", tcp_port));
            } else {
                warn!("TCP port {} is NOT reachable - check firewall/port forwarding", tcp_port);
                m.log(LogLevel::Warning, format!("TCP port {} NOT reachable - check firewall", tcp_port));
            }
        });
    } else {
        warn!("Could not detect public IP - clients may not be able to connect via relay");
        let mut m = metrics.write();
        m.log(LogLevel::Warning, "Could not detect public IP");
        let _ = event_tx.send(NetworkEvent::PublicIp(None));
    }

    // Track peer verification status
    // Peers must identify as Cider clients within the timeout or get disconnected
    let mut verified_peers: HashSet<PeerId> = HashSet::new();
    let mut pending_peers: HashMap<PeerId, Instant> = HashMap::new();

    // Create interval for checking pending peer timeouts
    let mut timeout_check = tokio::time::interval(Duration::from_secs(5));

    {
        let mut m = metrics.write();
        m.log(LogLevel::Info, "Cider-only mode: non-Cider peers will be rejected");
    }
    info!("Cider-only mode enabled: peers must identify as Cider clients");

    // Event loop
    loop {
        tokio::select! {
            // Check for timed-out pending peers
            _ = timeout_check.tick() => {
                let now = Instant::now();
                let timed_out: Vec<PeerId> = pending_peers
                    .iter()
                    .filter(|(_, connected_at)| now.duration_since(**connected_at).as_secs() > IDENTIFY_TIMEOUT_SECS)
                    .map(|(peer_id, _)| *peer_id)
                    .collect();

                for peer_id in timed_out {
                    pending_peers.remove(&peer_id);
                    let short_id = truncate_peer_id(&peer_id.to_string());
                    warn!("Disconnecting peer {} - failed to identify as Cider within {}s", short_id, IDENTIFY_TIMEOUT_SECS);
                    let _ = swarm.disconnect_peer_id(peer_id);

                    let mut m = metrics.write();
                    m.log(LogLevel::Warning, format!("Rejected: {} (identify timeout)", short_id));
                }
            }

            // Handle swarm events
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Listening on: {}", address);
                        let mut m = metrics.write();
                        m.log(LogLevel::Info, format!("Listening: {}", address));
                    }

                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        let short_id = truncate_peer_id(&peer_id.to_string());

                        // Skip if already verified (additional transport to same peer)
                        if verified_peers.contains(&peer_id) {
                            info!("Peer connected: {} (already verified, additional transport)", short_id);
                        } else {
                            info!("Peer connected: {} (pending verification)", short_id);
                            // Only add if not already pending (don't reset timeout)
                            pending_peers.entry(peer_id).or_insert(Instant::now());
                        }

                        let mut m = metrics.write();
                        m.connection_established(peer_id.to_string(), None);
                    }

                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        let short_id = truncate_peer_id(&peer_id.to_string());
                        info!("Peer disconnected: {}", short_id);

                        // Clean up tracking
                        verified_peers.remove(&peer_id);
                        pending_peers.remove(&peer_id);

                        let mut m = metrics.write();
                        m.connection_closed(&peer_id.to_string());
                    }

                    SwarmEvent::Behaviour(RelayServerBehaviourEvent::Relay(
                        relay::Event::ReservationReqAccepted { src_peer_id, .. },
                    )) => {
                        let short_id = truncate_peer_id(&src_peer_id.to_string());

                        // Log reservation - verification happens via identify
                        // If peer doesn't identify as Cider within timeout, they get disconnected anyway
                        if verified_peers.contains(&src_peer_id) {
                            info!("Relay reservation accepted: {} (verified)", short_id);
                        } else {
                            info!("Relay reservation accepted: {} (pending verification)", short_id);
                        }
                        let mut m = metrics.write();
                        m.reservation_accepted(&src_peer_id.to_string());
                    }

                    SwarmEvent::Behaviour(RelayServerBehaviourEvent::Relay(
                        relay::Event::CircuitReqAccepted {
                            src_peer_id,
                            dst_peer_id,
                            ..
                        },
                    )) => {
                        let src_short = truncate_peer_id(&src_peer_id.to_string());
                        let dst_short = truncate_peer_id(&dst_peer_id.to_string());
                        info!("Relay circuit: {} -> {}", src_short, dst_short);
                        let mut m = metrics.write();
                        m.circuit_established(&src_peer_id.to_string(), &dst_peer_id.to_string());
                    }

                    SwarmEvent::Behaviour(RelayServerBehaviourEvent::Relay(
                        relay::Event::CircuitClosed { .. },
                    )) => {
                        info!("Relay circuit closed");
                        let mut m = metrics.write();
                        m.circuit_closed();
                    }

                    SwarmEvent::Behaviour(RelayServerBehaviourEvent::Identify(
                        identify::Event::Received { peer_id, info, .. },
                    )) => {
                        let short_id = truncate_peer_id(&peer_id.to_string());
                        let is_cider = info.protocol_version.to_lowercase().contains(CIDER_PROTOCOL_PREFIX);

                        // Skip if already verified (identify can fire multiple times)
                        if verified_peers.contains(&peer_id) {
                            continue;
                        }

                        if is_cider {
                            // Verified as Cider client
                            pending_peers.remove(&peer_id);
                            verified_peers.insert(peer_id);

                            info!("Verified Cider peer: {} ({})", short_id, info.protocol_version);
                            let mut m = metrics.write();
                            m.peer_identified(&peer_id.to_string(), info.protocol_version.clone());
                            m.log(LogLevel::Info, format!("Verified: {} ({})", short_id, info.protocol_version));
                        } else {
                            // Not a Cider client - disconnect immediately
                            pending_peers.remove(&peer_id);

                            warn!("Rejecting non-Cider peer: {} ({})", short_id, info.protocol_version);
                            let _ = swarm.disconnect_peer_id(peer_id);

                            let mut m = metrics.write();
                            m.log(LogLevel::Warning, format!("Rejected: {} (non-Cider: {})", short_id, info.protocol_version));
                        }
                    }

                    // Suppress other events
                    _ => {}
                }
            }
        }
    }
}

/// Run with plain logging (no dashboard)
pub async fn run_with_logging(metrics: Arc<RwLock<Metrics>>) -> Result<(), Box<dyn Error>> {
    // Initialize tracing for logging mode
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cider_relay=info".parse()?)
                .add_directive("libp2p_relay=info".parse()?)
                .add_directive("libp2p_kad=warn".parse()?)
                .add_directive("libp2p_identify=warn".parse()?),
        )
        .init();

    let (tx, _rx) = mpsc::unbounded_channel();
    run_with_dashboard(metrics, tx).await
}

/// Detect public IP address using external services
async fn detect_public_ip() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    let services = [
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
        "https://icanhazip.com",
    ];

    for service in services {
        if let Ok(resp) = client.get(service).send().await {
            if let Ok(ip) = resp.text().await {
                let ip = ip.trim().to_string();
                if ip.contains('.') && ip.len() <= 15 && ip.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    return Some(ip);
                }
            }
        }
    }
    None
}

/// Check if a port is reachable from the internet
async fn check_port_reachable(ip: &str, port: u16) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    // portchecker.io requires POST with JSON body
    let body = format!(r#"{{"host":"{}","ports":[{}]}}"#, ip, port);

    match client
        .post("https://portchecker.io/api/v1/query")
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(text) = resp.text().await {
                // Response: {"check":[{"port":4001,"status":true}],...}
                text.contains("\"status\":true")
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
