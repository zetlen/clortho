use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

/// A passphrase or PIN. Zeroed on drop and never printed by `Debug`.
///
/// Serialization through serde_json makes transient copies that are not
/// zeroed; the type limits accidental exposure, not a memory-reading attacker.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Secret(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}
