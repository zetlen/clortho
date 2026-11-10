//! Shared pieces of clortho: the JSON-lines wire protocol spoken between the
//! daemon and its clients, a pinentry-compatible Assuan session parser, and
//! the local socket transport.

pub mod assuan;
pub mod proto;
pub mod secret;
pub mod transport;

pub use secret::Secret;
