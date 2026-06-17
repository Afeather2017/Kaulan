//! Device discovery module
//!
//! This module implements the Kaulan Discovery Protocol (KDP) for local network
//! device discovery using UDP broadcast.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Discovery Service                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────┐         ┌─────────────────────────┐   │
//! │  │  HTTP Trigger   │────────▶│ UDP Network (255.255.255.255) │
//! │  │ (manual refresh)│         │         Port 2082        │   │
//! │  └─────────────────┘         └─────────────────────────┘   │
//! │                                        ▲                    │
//! │                                        │                    │
//! │  ┌─────────────────┐                   │                    │
//! │  │   Listener      │───────────────────┘                    │
//! │  │ (req/resp loop) │                                        │
//! │  └─────────────────┘                                        │
//! │           │                                                  │
//! │           ▼                                                  │
//! │  ┌─────────────────────────┐                                │
//! │  │  Discovered Devices     │                                │
//! │  │ (scan transaction)      │                                │
//! │  └─────────────────────────┘                                │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Protocol Specification
//!
//! See [docs/device-discovery.md](../../../docs/device-discovery.md) for full protocol details.
//!
//! # Modules
//!
//! - [`types`] - Data structures for discovery messages and state
//! - [`socket`] - Shared UDP socket for discovery (single socket for send/receive)
//! - [`discovery`] - UDP listener and request sender logic

#[allow(clippy::module_inception)]
pub mod discovery;
pub mod socket;
pub mod types;

pub use types::{DiscoveredDevice, DiscoveryError, DiscoveryMessage, DiscoveryState};
