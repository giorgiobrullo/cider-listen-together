//! P2P Networking
//!
//! Uses libp2p for decentralized peer-to-peer connectivity.

mod behaviour;
mod room_code;
pub mod signaling;

pub use behaviour::{NetworkError, NetworkEvent, NetworkHandle, NetworkManager};
pub use room_code::RoomCode;
pub use signaling::SignalingClient;
