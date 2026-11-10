use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StorePolicy {
    /// Never read or write the key store.
    Off,
    /// Stored secrets are released only when a UI approves the request.
    #[default]
    Confirm,
    /// Stored secrets answer matching requests with no interaction.
    Auto,
}

impl StorePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            StorePolicy::Off => "off",
            StorePolicy::Confirm => "confirm",
            StorePolicy::Auto => "auto",
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StoreBackend {
    /// macOS Keychain, Windows Credential Manager, or the Secret Service.
    #[default]
    Os,
    /// Process memory only; forgotten when the daemon exits.
    Memory,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub store: StorePolicy,
    pub store_backend: StoreBackend,
    /// Resolved requests kept for display.
    pub history: usize,
    /// Applied when the requester sets no timeout. 0 waits forever.
    pub default_timeout_secs: u64,
    /// Pinentry to exec when the daemon cannot be reached.
    pub fallback_pinentry: Option<PathBuf>,
    /// Run on each new request unless a self-notifying UI is attached.
    /// `{title}` and `{body}` are substituted in arguments. Unset uses a
    /// platform default; an empty list disables it.
    pub notify_command: Option<Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            store: StorePolicy::default(),
            store_backend: StoreBackend::default(),
            history: 50,
            default_timeout_secs: 0,
            fallback_pinentry: None,
            notify_command: None,
        }
    }
}

pub fn path() -> PathBuf {
    if let Some(p) = std::env::var_os("CLORTHO_CONFIG") {
        return PathBuf::from(p);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clortho")
        .join("config.toml")
}

/// Directory for non-secret state (the index of remembered key names).
pub fn state_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("CLORTHO_STATE_DIR") {
        return PathBuf::from(p);
    }
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clortho")
}

impl Config {
    /// A missing file yields the defaults; a malformed one is an error.
    pub fn load() -> Result<Config, String> {
        let p = path();
        match std::fs::read_to_string(&p) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", p.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(format!("{}: {e}", p.display())),
        }
    }

    pub fn notify_argv(&self) -> Vec<String> {
        if let Some(cmd) = &self.notify_command {
            return cmd.clone();
        }
        let v: &[&str] = if cfg!(target_os = "macos") {
            // Text is passed as argv, never interpolated into the script.
            &[
                "osascript",
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                "{title}",
                "{body}",
            ]
        } else if cfg!(windows) {
            &[]
        } else {
            &["notify-send", "--app-name=clortho", "{title}", "{body}"]
        };
        v.iter().map(|s| s.to_string()).collect()
    }
}
