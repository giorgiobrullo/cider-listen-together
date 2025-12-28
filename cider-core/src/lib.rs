//! Cider Listen Together - Core Library
//!
//! This library provides the core functionality for syncing music playback
//! across multiple Cider instances via P2P networking.

pub mod cider;
pub mod ffi;
pub mod latency;
pub mod network;
pub mod seek_calibrator;
pub mod sync;

// Re-exports for convenience
pub use cider::{CiderClient, NowPlaying};
pub use sync::{Room, RoomState, SyncMessage};

// Setup uniffi scaffolding
uniffi::setup_scaffolding!();
