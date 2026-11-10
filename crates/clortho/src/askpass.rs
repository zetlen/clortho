//! `clortho-askpass`: for SSH_ASKPASS, SUDO_ASKPASS, GIT_ASKPASS and anything
//! else that runs a helper with the prompt as its argument and reads the
//! answer from the helper's stdout.

use crate::client;
use clortho_core::proto::{Kind, Origin, PinResult, Prompt, Role, Source};
use std::io::Write;

/// How the answer travels back to the caller.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Answer {
    /// Print the typed text.
    Text,
    /// Print "yes" or "no" (ssh host-key questions).
    YesNo,
    /// Exit status only.
    Status,
}

fn quoted(text: &str) -> Option<&str> {
    let start = text.find('\'')? + 1;
    let end = start + text[start..].find('\'')?;
    (end > start).then(|| &text[start..end])
}

/// Work out what is being asked from the prompt text. `hint` is the value
/// of `SSH_ASKPASS_PROMPT`, which OpenSSH sets to "confirm" or "none".
pub fn classify(text: &str, hint: Option<&str>) -> (Prompt, Answer) {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    let git_user = lower.starts_with("username for ");
    let git_pass = lower.starts_with("password for '");
    let source = if lower.starts_with("[sudo]") || lower.contains("sudo password") {
        Source::Sudo
    } else if git_user || git_pass {
        Source::Git
    } else if hint.is_some()
        || lower.contains("passphrase for")
        || lower.contains("'s password")
        || lower.contains("(yes/no")
        || lower.contains("enter pin for")
        || lower.contains("authenticity of host")
    {
        Source::Ssh
    } else {
        Source::Askpass
    };

    let (kind, answer) = match hint {
        Some("confirm") => (Kind::Confirm, Answer::Status),
        Some("none") => (Kind::Message, Answer::Status),
        _ if lower.contains("(yes/no") => (Kind::Confirm, Answer::YesNo),
        _ => (Kind::Pin, Answer::Text),
    };

    let mut p = Prompt::new(kind, source);
    p.description = Some(trimmed.to_string());
    if kind == Kind::Pin {
        match source {
            Source::Ssh => {
                if lower.contains("passphrase for") {
                    p.key = quoted(trimmed).map(String::from);
                    p.allow_store = p.key.is_some();
                } else if let Some(i) = lower.find("'s password") {
                    // Grouped, but login passwords are not offered to the store.
                    p.key = trimmed[..i]
                        .split_whitespace()
                        .last()
                        .map(|w| format!("password:{w}"));
                }
            }
            Source::Git => {
                p.key = quoted(trimmed).map(|q| {
                    if git_user {
                        format!("username:{q}")
                    } else {
                        q.to_string()
                    }
                });
                p.echo = git_user;
                p.allow_store = git_pass && p.key.is_some();
            }
            Source::Sudo => {
                p.key = Some(
                    lower
                        .split("password for ")
                        .nth(1)
                        .map(|u| u.trim_end_matches([':', ' ']).to_string())
                        .unwrap_or_else(|| "sudo".into()),
                );
            }
            _ => {}
        }
    }
    (p, answer)
}

fn origin() -> Origin {
    Origin {
        pid: parent_pid(),
        cwd: std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string()),
        ..Origin::default()
    }
}

#[cfg(unix)]
fn parent_pid() -> Option<u32> {
    Some(unsafe { libc::getppid() } as u32)
}

#[cfg(not(unix))]
fn parent_pid() -> Option<u32> {
    // The daemon reports this process's ancestors instead.
    Some(std::process::id())
}

pub fn run() -> i32 {
    let text = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let hint = std::env::var("SSH_ASKPASS_PROMPT").ok();
    let (prompt, answer) = classify(&text, hint.as_deref());
    let mut conn = match client::connect_or_spawn(Role::Requester, "askpass", false) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("clortho-askpass: cannot reach the clortho daemon: {e}");
            return 1;
        }
    };
    let result = match conn.request(prompt, origin()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("clortho-askpass: {e}");
            return 1;
        }
    };
    let mut out = std::io::stdout().lock();
    match (answer, result) {
        (Answer::Text, PinResult::Secret { secret, .. }) => {
            let ok = writeln!(out, "{}", secret.expose())
                .and_then(|_| out.flush())
                .is_ok();
            if ok { 0 } else { 1 }
        }
        (Answer::YesNo, PinResult::Confirmed) => {
            let _ = writeln!(out, "yes");
            0
        }
        (Answer::YesNo, PinResult::Denied) => {
            let _ = writeln!(out, "no");
            0
        }
        (Answer::Status, PinResult::Confirmed) => 0,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_key_passphrase() {
        let (p, a) = classify("Enter passphrase for key '/home/v/.ssh/id_ed25519': ", None);
        assert_eq!(
            (p.kind, p.source, a),
            (Kind::Pin, Source::Ssh, Answer::Text)
        );
        assert_eq!(p.key.as_deref(), Some("/home/v/.ssh/id_ed25519"));
        assert!(p.allow_store && !p.echo);
    }

    #[test]
    fn ssh_login_password_is_grouped_but_not_storable() {
        let (p, _) = classify("vinz@gozer.example's password: ", None);
        assert_eq!(p.source, Source::Ssh);
        assert_eq!(p.key.as_deref(), Some("password:vinz@gozer.example"));
        assert!(!p.allow_store);
    }

    #[test]
    fn ssh_host_key_and_confirm_hints() {
        let q = "The authenticity of host 'h (1.2.3.4)' can't be established.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
        let (p, a) = classify(q, None);
        assert_eq!(
            (p.kind, p.source, a),
            (Kind::Confirm, Source::Ssh, Answer::YesNo)
        );
        assert!(p.key.is_none());

        let (p, a) = classify("Allow use of key id_ed25519?", Some("confirm"));
        assert_eq!(
            (p.kind, p.source, a),
            (Kind::Confirm, Source::Ssh, Answer::Status)
        );
        let (p, a) = classify("Confirm user presence for key", Some("none"));
        assert_eq!((p.kind, a), (Kind::Message, Answer::Status));
    }

    #[test]
    fn sudo_and_git() {
        let (p, _) = classify("[sudo] password for vinz: ", None);
        assert_eq!(p.source, Source::Sudo);
        assert_eq!(p.key.as_deref(), Some("vinz"));
        assert!(!p.allow_store);

        let (p, _) = classify("Username for 'https://github.com': ", None);
        assert_eq!(p.source, Source::Git);
        assert!(p.echo && !p.allow_store);
        assert_eq!(p.key.as_deref(), Some("username:https://github.com"));

        let (p, _) = classify("Password for 'https://vinz@github.com': ", None);
        assert_eq!(p.source, Source::Git);
        assert_eq!(p.key.as_deref(), Some("https://vinz@github.com"));
        assert!(p.allow_store && !p.echo);
    }

    #[test]
    fn unknown_prompts_are_plain_pins() {
        let (p, a) = classify("Vault token:", None);
        assert_eq!(
            (p.kind, p.source, a),
            (Kind::Pin, Source::Askpass, Answer::Text)
        );
        assert!(p.key.is_none());
    }
}
