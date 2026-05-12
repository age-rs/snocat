// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license OR Apache 2.0

//! Runtime toggle for QUIC connection diagnostic logging.
//!
//! By default, QUIC connection lifecycle logging is **disabled**. Library consumers
//! can enable it at runtime to get detailed `tracing` events for connection
//! establishment, drops, timeouts, resets, and other QUIC-specific conditions.
//!
//! # Usage
//!
//! ```rust
//! // Enable QUIC connection logging at application startup
//! snocat::quic_logging::enable();
//!
//! // Disable it again if needed
//! snocat::quic_logging::disable();
//!
//! // Check current state
//! if snocat::quic_logging::is_enabled() {
//!     println!("QUIC connection logging is active");
//! }
//! ```
//!
//! The emitted events use the `tracing` crate at appropriate severity levels
//! (`error`, `warn`, `info`, `debug`) and are only generated when logging is
//! enabled, so there is negligible overhead when disabled.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable QUIC connection diagnostic logging.
///
/// Once enabled, the library emits `tracing` events for QUIC connection
/// lifecycle events such as connection creation, handshake failures,
/// idle timeouts, transport errors, stateless resets, and graceful closures.
pub fn enable() {
  ENABLED.store(true, Ordering::Relaxed);
}

/// Disable QUIC connection diagnostic logging.
pub fn disable() {
  ENABLED.store(false, Ordering::Relaxed);
}

/// Returns `true` if QUIC connection diagnostic logging is currently enabled.
pub fn is_enabled() -> bool {
  ENABLED.load(Ordering::Relaxed)
}
