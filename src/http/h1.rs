//! Minimal, low-level HTTP 1.1 protocol implementation
//!
//! This module is sans-IO; it doesn't use networking sockets.
pub mod codec;
pub mod error;
pub mod header;
