//! Server side of the pinentry dialect of the Assuan protocol, as a pure
//! state machine: feed it lines, it says what to write or what to ask.

use crate::proto::{Kind, Origin, PinResult, Prompt, Source};

pub const GREETING: &str = "OK Pleased to meet you";

// gpg-error codes with GPG_ERR_SOURCE_PINENTRY (5 << 24).
const ERR_GENERAL: &str = "ERR 83886081 General error <Pinentry>";
const ERR_TIMEOUT: &str = "ERR 83886142 Timeout <Pinentry>";
const ERR_CANCELED: &str = "ERR 83886179 Operation cancelled <Pinentry>";
const ERR_NOT_CONFIRMED: &str = "ERR 83886194 Not confirmed <Pinentry>";
const ERR_UNKNOWN_CMD: &str = "ERR 83886355 Unknown IPC command <Pinentry>";
const ERR_PARAMETER: &str = "ERR 83886360 IPC parameter error <Pinentry>";

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Write these lines and continue.
    Reply(Vec<String>),
    /// Ask the user, then pass the result to [`Session::finish`].
    Ask(Kind),
    /// Write these lines and end the session.
    Bye(Vec<String>),
}

#[derive(Debug, Default)]
pub struct Session {
    title: Option<String>,
    desc: Option<String>,
    prompt: Option<String>,
    error: Option<String>,
    keyinfo: Option<String>,
    ok: Option<String>,
    cancel: Option<String>,
    notok: Option<String>,
    repeat: bool,
    timeout: Option<u64>,
    allow_external_cache: bool,
    one_button: bool,
    saw_owner: bool,
    pub origin: Origin,
}

fn ok() -> Action {
    Action::Reply(vec!["OK".into()])
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&mut self, line: &str) -> Action {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() || line.starts_with('#') {
            return Action::Reply(vec![]);
        }
        let (cmd, arg) = match line.split_once(' ') {
            Some((c, a)) => (c, a.trim_start()),
            None => (line, ""),
        };
        let text = || {
            if arg.is_empty() {
                None
            } else {
                Some(unescape(arg))
            }
        };
        match cmd.to_ascii_uppercase().as_str() {
            "SETTITLE" => self.title = text(),
            "SETDESC" => self.desc = text(),
            "SETPROMPT" => self.prompt = text(),
            "SETERROR" => self.error = text(),
            "SETOK" => self.ok = text(),
            "SETCANCEL" => self.cancel = text(),
            "SETNOTOK" => self.notok = text(),
            "SETKEYINFO" => self.keyinfo = text().filter(|k| k != "--clear"),
            "SETREPEAT" => self.repeat = true,
            "SETTIMEOUT" => match arg.trim().parse::<u64>() {
                Ok(n) => self.timeout = (n > 0).then_some(n),
                Err(_) => return Action::Reply(vec![ERR_PARAMETER.into()]),
            },
            "OPTION" => self.option(arg),
            "GETINFO" => return self.getinfo(arg),
            "GETPIN" => return Action::Ask(Kind::Pin),
            "CONFIRM" => {
                self.one_button = arg.contains("--one-button");
                return Action::Ask(if self.one_button {
                    Kind::Message
                } else {
                    Kind::Confirm
                });
            }
            "MESSAGE" => {
                self.one_button = true;
                return Action::Ask(Kind::Message);
            }
            // Accepted and ignored: cosmetic, or meaningless without a local dialog.
            "SETQUALITYBAR" | "SETQUALITYBAR_TT" | "SETREPEATERROR" | "SETREPEATOK"
            | "SETGENPIN" | "SETGENPIN_TT" | "CLEARPASSPHRASE" | "NOP" | "RESET" | "END"
            | "CANCEL" => {}
            "BYE" => return Action::Bye(vec!["OK closing connection".into()]),
            "HELP" => {
                return Action::Reply(vec![
                    "# clortho speaks the pinentry protocol; see pinentry(1)".into(),
                    "OK".into(),
                ]);
            }
            _ => return Action::Reply(vec![ERR_UNKNOWN_CMD.into()]),
        }
        ok()
    }

    fn option(&mut self, arg: &str) {
        let arg = arg.trim_start_matches("--");
        let (name, value) = match arg.split_once(['=', ' ']) {
            Some((n, v)) => (n.trim(), v.trim()),
            None => (arg.trim(), ""),
        };
        match name {
            "ttyname" => self.origin.tty = Some(value.to_string()).filter(|v| !v.is_empty()),
            "timeout" => self.timeout = value.parse().ok().filter(|n| *n > 0),
            "allow-external-password-cache" => self.allow_external_cache = true,
            // owner=<pid>[/<uid>] <hostname>: the process that asked gpg-agent.
            "owner" => {
                self.saw_owner = true;
                let (ids, host) = value.split_once(' ').unwrap_or((value, ""));
                let (pid, uid) = ids.split_once('/').unwrap_or((ids, ""));
                self.origin.pid = pid.parse().ok();
                self.origin.uid = uid.parse().ok();
                self.origin.host = Some(host.trim().to_string()).filter(|h| !h.is_empty());
            }
            _ => {}
        }
    }

    fn getinfo(&self, arg: &str) -> Action {
        let d = match arg.trim() {
            "flavor" => "clortho".to_string(),
            "version" => env!("CARGO_PKG_VERSION").to_string(),
            "pid" => std::process::id().to_string(),
            "ttyinfo" => "- - -".to_string(),
            _ => return Action::Reply(vec![ERR_PARAMETER.into()]),
        };
        Action::Reply(vec![format!("D {d}"), "OK".into()])
    }

    /// The request to forward for an [`Action::Ask`].
    pub fn to_prompt(&self, kind: Kind) -> Prompt {
        let from_gpg = self.saw_owner
            || self
                .keyinfo
                .as_deref()
                .is_some_and(|k| k.as_bytes().get(1) == Some(&b'/'));
        let mut p = Prompt::new(
            kind,
            if from_gpg {
                Source::Gpg
            } else {
                Source::Pinentry
            },
        );
        p.title = self.title.clone();
        p.description = self.desc.clone();
        p.prompt = self.prompt.clone();
        p.error = self.error.clone();
        p.key = self.keyinfo.clone();
        p.repeat = kind == Kind::Pin && self.repeat;
        p.allow_store = kind == Kind::Pin && self.allow_external_cache && self.keyinfo.is_some();
        p.ok_label = self.ok.clone();
        p.cancel_label = self.cancel.clone();
        p.notok_label = self.notok.clone();
        p.timeout_secs = self.timeout;
        p
    }

    /// Lines to write once the user (or daemon) has answered. `None` means
    /// the daemon could not be reached.
    pub fn finish(&mut self, kind: Kind, result: Option<PinResult>) -> Vec<String> {
        let repeated = self.repeat;
        // Error text and repeat mode apply to a single interaction.
        self.error = None;
        self.repeat = false;
        match (kind, result) {
            (_, None) => vec![ERR_GENERAL.into()],
            (Kind::Pin, Some(PinResult::Secret { secret, from_store })) => {
                let mut out = Vec::new();
                if from_store {
                    out.push("S PASSWORD_FROM_CACHE".into());
                }
                if repeated {
                    out.push("S PIN_REPEATED".into());
                }
                if !secret.is_empty() {
                    out.push(format!("D {}", escape(secret.expose())));
                }
                out.push("OK".into());
                out
            }
            (Kind::Confirm | Kind::Message, Some(PinResult::Confirmed)) => vec!["OK".into()],
            (_, Some(PinResult::Denied)) => vec![ERR_NOT_CONFIRMED.into()],
            (_, Some(PinResult::TimedOut)) => vec![ERR_TIMEOUT.into()],
            (_, Some(_)) => vec![ERR_CANCELED.into()],
        }
    }
}

/// Decode Assuan percent escapes.
pub fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Encode data for a `D` line.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Secret;

    #[test]
    fn escapes_round_trip() {
        assert_eq!(unescape("a%0Ab%25c"), "a\nb%c");
        assert_eq!(unescape("100%"), "100%");
        assert_eq!(unescape("%zz%4"), "%zz%4");
        assert_eq!(escape("p%ss\nword"), "p%25ss%0Aword");
        assert_eq!(unescape(&escape("p%ss\r\nw%0Ard")), "p%ss\r\nw%0Ard");
    }

    #[test]
    fn gpg_agent_session() {
        let mut s = Session::new();
        for l in [
            "OPTION no-grab",
            "OPTION ttyname=/dev/pts/3",
            "OPTION allow-external-password-cache",
            "OPTION owner=4242/1000 box",
            "SETKEYINFO n/ABCDEF0123",
            "SETDESC Please enter the passphrase%0A%22Vinz <v@gozer>%22",
            "SETPROMPT Passphrase:",
        ] {
            assert_eq!(s.handle(l), ok(), "{l}");
        }
        assert_eq!(
            s.handle("GETINFO flavor"),
            Action::Reply(vec!["D clortho".into(), "OK".into()])
        );
        assert_eq!(s.handle("GETPIN"), Action::Ask(Kind::Pin));
        let p = s.to_prompt(Kind::Pin);
        assert_eq!(p.source, Source::Gpg);
        assert_eq!(p.key.as_deref(), Some("n/ABCDEF0123"));
        assert!(p.allow_store);
        assert_eq!(p.summary(), "key \"Vinz <v@gozer>\"");
        assert_eq!(s.origin.pid, Some(4242));
        assert_eq!(s.origin.uid, Some(1000));
        assert_eq!(s.origin.host.as_deref(), Some("box"));
        assert_eq!(s.origin.tty.as_deref(), Some("/dev/pts/3"));
        let out = s.finish(
            Kind::Pin,
            Some(PinResult::Secret {
                secret: Secret::new("hunter2%"),
                from_store: false,
            }),
        );
        assert_eq!(out, vec!["D hunter2%25", "OK"]);
        assert_eq!(
            s.handle("BYE"),
            Action::Bye(vec!["OK closing connection".into()])
        );
    }

    #[test]
    fn errors_and_flags() {
        let mut s = Session::new();
        s.handle("SETERROR Bad Passphrase (try 2 of 3)");
        s.handle("SETREPEAT");
        let p = s.to_prompt(Kind::Pin);
        assert!(p.repeat);
        assert_eq!(p.error.as_deref(), Some("Bad Passphrase (try 2 of 3)"));
        assert_eq!(p.source, Source::Pinentry);
        let out = s.finish(
            Kind::Pin,
            Some(PinResult::Secret {
                secret: Secret::new("x"),
                from_store: true,
            }),
        );
        assert_eq!(
            out,
            vec!["S PASSWORD_FROM_CACHE", "S PIN_REPEATED", "D x", "OK"]
        );
        // one-shot state is cleared
        let p = s.to_prompt(Kind::Pin);
        assert!(!p.repeat && p.error.is_none());

        assert_eq!(
            s.finish(Kind::Pin, Some(PinResult::Cancelled)),
            vec![ERR_CANCELED]
        );
        assert_eq!(
            s.finish(Kind::Pin, Some(PinResult::TimedOut)),
            vec![ERR_TIMEOUT]
        );
        assert_eq!(
            s.finish(Kind::Confirm, Some(PinResult::Denied)),
            vec![ERR_NOT_CONFIRMED]
        );
        assert_eq!(
            s.finish(Kind::Confirm, Some(PinResult::Confirmed)),
            vec!["OK"]
        );
        assert_eq!(s.finish(Kind::Pin, None), vec![ERR_GENERAL]);
        assert_eq!(s.handle("CONFIRM --one-button"), Action::Ask(Kind::Message));
        assert_eq!(s.handle("CONFIRM"), Action::Ask(Kind::Confirm));
        assert_eq!(
            s.handle("FROBNICATE"),
            Action::Reply(vec![ERR_UNKNOWN_CMD.into()])
        );
        assert_eq!(s.handle("SETTIMEOUT 30"), ok());
        assert_eq!(s.to_prompt(Kind::Pin).timeout_secs, Some(30));
    }
}
