//! Wire protocol, version 1. One JSON object per line in each direction.
//! See docs/PROTOCOL.md for the prose description.

use crate::secret::Secret;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A secret must be typed (or released from a key store).
    Pin,
    /// Yes/no question.
    Confirm,
    /// Information only; any acknowledgement resolves it.
    Message,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Gpg,
    Ssh,
    Sudo,
    Git,
    /// Some other program speaking the pinentry protocol (rbw, age plugins...).
    Pinentry,
    /// Some other program using the askpass convention.
    Askpass,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Gpg => "gpg",
            Source::Ssh => "ssh",
            Source::Sudo => "sudo",
            Source::Git => "git",
            Source::Pinentry => "pinentry",
            Source::Askpass => "askpass",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub kind: Kind,
    pub source: Source,
    pub title: Option<String>,
    pub description: Option<String>,
    pub prompt: Option<String>,
    /// Set when the previous answer was rejected ("Bad passphrase").
    pub error: Option<String>,
    /// Stable identity of the thing being unlocked (gpg keygrip, ssh key
    /// path, git URL). Requests with the same source and key are grouped and
    /// can be answered together; it is also the key-store lookup key.
    pub key: Option<String>,
    /// A new secret is being set; UIs should ask twice.
    #[serde(default)]
    pub repeat: bool,
    /// The answer is not secret (a username); UIs may show it.
    #[serde(default)]
    pub echo: bool,
    /// The requester permits keeping the secret in an external store.
    #[serde(default)]
    pub allow_store: bool,
    pub ok_label: Option<String>,
    pub cancel_label: Option<String>,
    pub notok_label: Option<String>,
    pub timeout_secs: Option<u64>,
}

impl Prompt {
    pub fn new(kind: Kind, source: Source) -> Self {
        Prompt {
            kind,
            source,
            title: None,
            description: None,
            prompt: None,
            error: None,
            key: None,
            repeat: false,
            echo: false,
            allow_store: false,
            ok_label: None,
            cancel_label: None,
            notok_label: None,
            timeout_secs: None,
        }
    }

    /// One line describing the request, for lists and notifications.
    pub fn summary(&self) -> String {
        let text = self
            .description
            .as_deref()
            .or(self.prompt.as_deref())
            .or(self.title.as_deref())
            .unwrap_or("");
        // gpg puts the user id on its own quoted line; prefer it.
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if self.source == Source::Gpg
            && let Some(uid) = lines.iter().find(|l| l.starts_with('"') && l.len() > 2)
        {
            return format!("key {}", uid);
        }
        lines
            .first()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "(no description)".into())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
}

/// Where a request came from. The requester fills what it knows; the daemon
/// fills `chain` (the process and its ancestors, nearest first).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Origin {
    pub pid: Option<u32>,
    pub uid: Option<u32>,
    pub host: Option<String>,
    pub tty: Option<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub chain: Vec<ProcInfo>,
}

impl Origin {
    /// Short "who is asking" string: process names, requester first.
    pub fn who(&self) -> String {
        if !self.chain.is_empty() {
            let names: Vec<&str> = self.chain.iter().take(4).map(|p| p.name.as_str()).collect();
            return names.join(" ← ");
        }
        match self.pid {
            Some(pid) => format!("pid {pid}"),
            None => "unknown".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RequestView {
    pub id: u64,
    /// Requests sharing a group are answered together.
    pub group: String,
    pub prompt: Prompt,
    pub origin: Origin,
    /// Unix time, milliseconds.
    pub created_ms: u64,
    /// The key store holds a secret for this group.
    pub stored: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Answered,
    FromStore,
    Confirmed,
    Denied,
    Cancelled,
    TimedOut,
    /// The requesting process went away before an answer.
    Withdrawn,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub request: RequestView,
    pub outcome: Outcome,
    /// Name of the UI client that resolved it, or "store" / "daemon".
    pub by: String,
    pub resolved_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Requester,
    Ui,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Reply {
    /// Supply the secret. `remember` saves it to the key store if allowed.
    Secret {
        secret: Secret,
        #[serde(default)]
        remember: bool,
    },
    /// Release the secret already in the key store.
    UseStored,
    Confirm,
    Deny,
    Cancel,
    /// Delete the stored secret for this request's group; leaves it pending.
    Forget,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ClientMsg {
    Hello {
        version: u32,
        role: Role,
        name: String,
        /// UI only: this client raises its own notifications, so the daemon
        /// need not run its notify command while it is attached.
        #[serde(default)]
        notifies: bool,
    },
    /// Requester only. One outstanding request per connection.
    Request { prompt: Prompt, origin: Origin },
    /// UI only.
    Answer { id: u64, reply: Reply },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PinResult {
    Secret {
        secret: Secret,
        #[serde(default)]
        from_store: bool,
    },
    Confirmed,
    Denied,
    Cancelled,
    TimedOut,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        version: u32,
        daemon_pid: u32,
        /// Key-store policy: "off", "confirm" or "auto".
        store: String,
    },
    /// UI only: the complete state, sent on connect and after every change.
    Snapshot {
        pending: Vec<RequestView>,
        recent: Vec<HistoryEntry>,
    },
    /// Requester only.
    Result {
        result: PinResult,
    },
    Error {
        message: String,
    },
}
