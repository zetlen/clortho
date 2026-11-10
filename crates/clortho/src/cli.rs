use crate::client;
use crate::config::Config;
use clortho_core::Secret;
use clortho_core::proto::*;
use clortho_core::transport;
use std::io::BufRead;

const HELP: &str = "\
clortho - pinentry and askpass broker

USAGE
  clortho [tui] [--once]     open the request list (--once: exit when the queue empties)
  clortho daemon             run the broker in the foreground
  clortho status [--json]    print pending and recent requests
  clortho answer ID [--remember]
                             answer a request with a secret read from stdin
  clortho approve ID         confirm a yes/no request, or release the stored secret
  clortho deny ID            refuse a yes/no request
  clortho cancel ID          cancel a request
  clortho forget ID          delete the stored secret for a request's key
  clortho setup              print the configuration needed to route prompts here
  clortho paths              print socket, config and state locations

The shims are separate binaries: pinentry-clortho and clortho-askpass.
";

pub fn main() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    match args.first().map(String::as_str) {
        None | Some("tui") | Some("--once") => crate::tui::run(flag("--once")),
        Some("daemon") => match Config::load() {
            Ok(cfg) => match crate::daemon::run(cfg) {
                Ok(()) => 0,
                Err(e) => fail(&e.to_string()),
            },
            Err(e) => fail(&e),
        },
        Some("status") => status(flag("--json")),
        Some(cmd @ ("answer" | "approve" | "deny" | "cancel" | "forget")) => {
            let Some(id) = args.get(1).and_then(|a| a.parse::<u64>().ok()) else {
                return fail("expected a numeric request ID");
            };
            act(cmd, id, flag("--remember"))
        }
        Some("setup") => {
            setup();
            0
        }
        Some("paths") => {
            println!("socket  {}", transport::socket_path().display());
            println!("config  {}", crate::config::path().display());
            println!("state   {}", crate::config::state_dir().display());
            0
        }
        Some("pinentry") => crate::pinentry::run(),
        Some("-V" | "--version" | "version") => {
            println!("clortho {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("-h" | "--help" | "help") => {
            print!("{HELP}");
            0
        }
        Some(other) => {
            eprintln!("clortho: unknown command '{other}'\n");
            eprint!("{HELP}");
            2
        }
    }
}

fn fail(msg: &str) -> i32 {
    eprintln!("clortho: {msg}");
    1
}

pub fn age(ms: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(ms);
    let s = now.saturating_sub(ms) / 1000;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m", s / 60),
        3600..86400 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

pub fn outcome_str(o: Outcome) -> &'static str {
    match o {
        Outcome::Answered => "answered",
        Outcome::FromStore => "from store",
        Outcome::Confirmed => "confirmed",
        Outcome::Denied => "denied",
        Outcome::Cancelled => "cancelled",
        Outcome::TimedOut => "timed out",
        Outcome::Withdrawn => "withdrawn",
    }
}

fn status(json: bool) -> i32 {
    let mut conn = match client::connect(Role::Ui, "status", false) {
        Ok(c) => c,
        Err(e) => return fail(&format!("daemon not reachable: {e}")),
    };
    let (pending, recent) = match conn.snapshot() {
        Ok(s) => s,
        Err(e) => return fail(&e.to_string()),
    };
    if json {
        let v = serde_json::json!({ "pending": pending, "recent": recent });
        println!("{v}");
        return 0;
    }
    println!(
        "daemon pid {}, store policy {}",
        conn.daemon_pid, conn.store_policy
    );
    println!("pending: {}", pending.len());
    for r in &pending {
        println!(
            "  #{:<4} {:>4}  {:<8} {}{}  [{}]",
            r.id,
            age(r.created_ms),
            r.prompt.source.as_str(),
            r.prompt.summary(),
            if r.stored { "  (stored)" } else { "" },
            r.origin.who(),
        );
    }
    println!("recent: {}", recent.len());
    for h in &recent {
        println!(
            "  #{:<4} {:>4}  {:<8} {:<10} {}",
            h.request.id,
            age(h.resolved_ms),
            h.request.prompt.source.as_str(),
            outcome_str(h.outcome),
            h.request.prompt.summary(),
        );
    }
    0
}

fn act(cmd: &str, id: u64, remember: bool) -> i32 {
    let mut conn = match client::connect(Role::Ui, "cli", false) {
        Ok(c) => c,
        Err(e) => return fail(&format!("daemon not reachable: {e}")),
    };
    let (pending, _) = match conn.snapshot() {
        Ok(s) => s,
        Err(e) => return fail(&e.to_string()),
    };
    let Some(req) = pending.iter().find(|r| r.id == id) else {
        return fail(&format!("request #{id} is not pending"));
    };
    let reply = match cmd {
        "answer" => {
            let mut line = String::new();
            if std::io::stdin().lock().read_line(&mut line).is_err() {
                return fail("could not read the secret from stdin");
            }
            let secret = Secret::new(line.trim_end_matches(['\r', '\n']));
            zeroize::Zeroize::zeroize(&mut line);
            Reply::Secret { secret, remember }
        }
        "approve" if req.prompt.kind == Kind::Pin => Reply::UseStored,
        "approve" => Reply::Confirm,
        "deny" => Reply::Deny,
        "cancel" => Reply::Cancel,
        _ => Reply::Forget,
    };
    if let Err(e) = conn.send(&ClientMsg::Answer { id, reply }) {
        return fail(&e.to_string());
    }
    // The daemon replies with an error, or with a snapshot reflecting the change.
    match conn.recv() {
        Ok(Some(ServerMsg::Error { message })) => fail(&message),
        Ok(Some(_)) => 0,
        Ok(None) => fail("daemon went away"),
        Err(e) => fail(&e.to_string()),
    }
}

fn setup() {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_default();
    let exe = std::env::consts::EXE_SUFFIX;
    let pinentry = dir.join(format!("pinentry-clortho{exe}"));
    let askpass = dir.join(format!("clortho-askpass{exe}"));
    println!(
        "\
# GnuPG (gpg, gpg-agent's ssh support, smartcard PINs): ~/.gnupg/gpg-agent.conf
pinentry-program {pinentry}
#   then: gpgconf --kill gpg-agent

# OpenSSH (ssh, ssh-add, git over ssh): shell profile
export SSH_ASKPASS={askpass}
export SSH_ASKPASS_REQUIRE=force     # 'prefer' keeps tty prompts when no DISPLAY

# sudo: shell profile, then use `sudo -A` (or alias sudo='sudo -A')
export SUDO_ASKPASS={askpass}

# git over https, when no credential helper answers first
git config --global core.askPass {askpass}

# Anything else that runs `pinentry` from PATH (rbw, age plugins, pass):
#   point its pinentry setting at {pinentry}

# Summon the list from tmux:
#   bind-key P display-popup -E 'clortho --once'",
        pinentry = pinentry.display(),
        askpass = askpass.display(),
    );
}
