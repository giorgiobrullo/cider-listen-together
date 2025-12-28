//! P2P Networking
//!
//! Uses libp2p for decentralized peer-to-peer connectivity.

mod behaviour;
mod room_code;

pub use behaviour::{NetworkError, NetworkEvent, NetworkHandle, NetworkManager};
pub use room_code::RoomCode;
