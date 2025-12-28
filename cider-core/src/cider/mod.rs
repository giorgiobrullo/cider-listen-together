//! Cider API Client
//!
//! This module provides a client for interacting with Cider's REST API.

mod client;
mod types;

pub use client::{CiderClient, CiderError};
pub use types::*;
