//! `pinentry-clortho`: what gpg-agent (or anything else that drives a
//! pinentry) executes. Translates the Assuan session into daemon requests.

use crate::client;
use crate::config::Config;
use clortho_core::assuan::{Action, GREETING, Session};
use clortho_core::proto::Role;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use zeroize::Zeroize;

fn write_lines(out: &mut impl Write, mut lines: Vec<String>) -> io::Result<()> {
    let mut r = Ok(());
    for l in &lines {
        r = r.and_then(|_| writeln!(out, "{l}"));
    }
    for l in &mut lines {
        l.zeroize();
    }
    r.and_then(|_| out.flush())
}

pub fn run() -> i32 {
    // Connect before greeting so that, on failure, a stock pinentry can take
    // over stdin/stdout untouched.
    let mut conn = match client::connect_or_spawn(Role::Requester, "pinentry", false) {
        Ok(c) => c,
        Err(e) => return fallback(&e),
    };
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut out = io::stdout().lock();
    if write_lines(&mut out, vec![GREETING.into()]).is_err() {
        return 1;
    }
    let mut session = Session::new();
    let mut raw = Vec::new();
    loop {
        raw.clear();
        match input.read_until(b'\n', &mut raw) {
            Ok(0) | Err(_) => return 0,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&raw);
        let lines = match session.handle(&line) {
            Action::Reply(lines) => lines,
            Action::Ask(kind) => {
                let prompt = session.to_prompt(kind);
                let result = conn.request(prompt, session.origin.clone()).ok();
                session.finish(kind, result)
            }
            Action::Bye(lines) => {
                let _ = write_lines(&mut out, lines);
                return 0;
            }
        };
        if write_lines(&mut out, lines).is_err() {
            return 1;
        }
    }
}

const STOCK: &[&str] = &[
    "pinentry-mac",
    "pinentry-gnome3",
    "pinentry-qt",
    "pinentry-gtk-2",
    "pinentry-curses",
    "pinentry-tty",
    "pinentry",
];

fn find_stock(cfg: &Config) -> Option<PathBuf> {
    if let Some(p) = &cfg.fallback_pinentry {
        return Some(p.clone());
    }
    let me = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok());
    let path = std::env::var_os("PATH")?;
    for name in STOCK {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            let Ok(real) = cand.canonicalize() else {
                continue;
            };
            let is_me = me.as_ref() == Some(&real)
                || real
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().contains("clortho"));
            if !is_me {
                return Some(cand);
            }
        }
    }
    None
}

fn fallback(why: &io::Error) -> i32 {
    let cfg = Config::load().unwrap_or_default();
    let Some(stock) = find_stock(&cfg) else {
        eprintln!("pinentry-clortho: daemon unreachable ({why}) and no fallback pinentry found");
        return 2;
    };
    eprintln!(
        "pinentry-clortho: daemon unreachable ({why}); using {}",
        stock.display()
    );
    let mut cmd = std::process::Command::new(stock);
    cmd.args(std::env::args_os().skip(1));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        eprintln!("pinentry-clortho: exec failed: {err}");
        2
    }
    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(s) => s.code().unwrap_or(2),
            Err(e) => {
                eprintln!("pinentry-clortho: {e}");
                2
            }
        }
    }
}
