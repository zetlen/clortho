//! The broker: accepts requests from shims, shows them to every attached
//! UI, and routes one answer to every request it satisfies.

use crate::config::{Config, StorePolicy};
use crate::store::{self, Store};
use clortho_core::proto::*;
use clortho_core::transport::{self, UnixListener, UnixStream};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{self, BufReader};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Pending {
    view: RequestView,
    conn: u64,
    tx: Sender<ServerMsg>,
    deadline: Option<Instant>,
}

struct Ui {
    notifies: bool,
    tx: Sender<ServerMsg>,
}

pub struct Broker {
    cfg: Config,
    store: Box<dyn Store>,
    next_id: u64,
    pending: BTreeMap<u64, Pending>,
    recent: VecDeque<HistoryEntry>,
    uis: HashMap<u64, Ui>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Broker {
    pub fn new(cfg: Config, store: Box<dyn Store>) -> Self {
        Broker {
            cfg,
            store,
            next_id: 1,
            pending: BTreeMap::new(),
            recent: VecDeque::new(),
            uis: HashMap::new(),
        }
    }

    fn storable(&self, p: &Prompt) -> bool {
        self.cfg.store != StorePolicy::Off
            && p.kind == Kind::Pin
            && p.allow_store
            && !p.repeat
            && !p.echo
            && p.key.is_some()
    }

    fn snapshot(&self) -> ServerMsg {
        ServerMsg::Snapshot {
            pending: self.pending.values().map(|p| p.view.clone()).collect(),
            recent: self.recent.iter().cloned().collect(),
        }
    }

    fn broadcast(&mut self) {
        let snap = self.snapshot();
        self.uis.retain(|_, ui| ui.tx.send(snap.clone()).is_ok());
    }

    fn record(&mut self, view: RequestView, outcome: Outcome, by: &str) {
        self.recent.push_front(HistoryEntry {
            request: view,
            outcome,
            by: by.to_string(),
            resolved_ms: now_ms(),
        });
        self.recent.truncate(self.cfg.history);
    }

    fn resolve(&mut self, id: u64, result: PinResult, outcome: Outcome, by: &str) {
        if let Some(p) = self.pending.remove(&id) {
            let _ = p.tx.send(ServerMsg::Result { result });
            self.record(p.view, outcome, by);
        }
    }

    /// Queue a request. Returns the view if a human now needs to act on it.
    pub fn submit(
        &mut self,
        conn: u64,
        prompt: Prompt,
        origin: Origin,
        tx: Sender<ServerMsg>,
    ) -> Option<RequestView> {
        // One outstanding request per connection.
        self.withdraw_conn_quiet(conn);
        let id = self.next_id;
        self.next_id += 1;
        let group = match &prompt.key {
            Some(k) => format!("{}:{}", prompt.source.as_str(), k),
            None => format!("#{id}"),
        };
        let storable = self.storable(&prompt);
        // The requester reports the last answer was wrong: a stored secret
        // for this key is stale.
        if prompt.error.is_some() && prompt.key.is_some() && self.cfg.store != StorePolicy::Off {
            self.store.delete(&group);
        }
        let mut stored = storable && self.store.has(&group);
        let timeout = prompt.timeout_secs.unwrap_or(self.cfg.default_timeout_secs);
        let deadline = (timeout > 0).then(|| Instant::now() + Duration::from_secs(timeout));
        let mut view = RequestView {
            id,
            group,
            prompt,
            origin,
            created_ms: now_ms(),
            stored,
        };

        if stored && self.cfg.store == StorePolicy::Auto {
            match self.store.get(&view.group) {
                Ok(Some(secret)) => {
                    let _ = tx.send(ServerMsg::Result {
                        result: PinResult::Secret {
                            secret,
                            from_store: true,
                        },
                    });
                    self.record(view, Outcome::FromStore, "store");
                    self.broadcast();
                    return None;
                }
                _ => stored = false,
            }
            view.stored = stored;
        }
        self.pending.insert(
            id,
            Pending {
                view: view.clone(),
                conn,
                tx,
                deadline,
            },
        );
        self.broadcast();
        Some(view)
    }

    /// Apply a UI's reply. Secrets and cancellations apply to the whole group.
    pub fn answer(&mut self, by: &str, id: u64, reply: Reply) -> Result<(), String> {
        let target = self
            .pending
            .get(&id)
            .ok_or("that request is no longer pending")?
            .view
            .clone();
        let group_ids: Vec<u64> = self
            .pending
            .values()
            .filter(|p| p.view.group == target.group && p.view.prompt.kind == target.prompt.kind)
            .map(|p| p.view.id)
            .collect();
        let mut warning = None;
        match (target.prompt.kind, reply) {
            (Kind::Pin, Reply::Secret { secret, remember }) => {
                if remember
                    && self.storable(&target.prompt)
                    && let Err(e) = self.store.set(&target.group, &secret)
                {
                    warning = Some(format!(
                        "answered, but the key store refused the secret: {e}"
                    ));
                }
                for gid in group_ids {
                    let result = PinResult::Secret {
                        secret: secret.clone(),
                        from_store: false,
                    };
                    self.resolve(gid, result, Outcome::Answered, by);
                }
            }
            (Kind::Pin, Reply::UseStored) => {
                if !target.stored || self.cfg.store == StorePolicy::Off {
                    return Err("no stored secret for this request".into());
                }
                match self.store.get(&target.group) {
                    Ok(Some(secret)) => {
                        for gid in group_ids {
                            let result = PinResult::Secret {
                                secret: secret.clone(),
                                from_store: true,
                            };
                            self.resolve(gid, result, Outcome::FromStore, by);
                        }
                    }
                    Ok(None) => {
                        self.refresh_stored();
                        self.broadcast();
                        return Err("the stored secret has disappeared from the key store".into());
                    }
                    Err(e) => return Err(format!("key store: {e}")),
                }
            }
            (Kind::Confirm | Kind::Message, Reply::Confirm) => {
                self.resolve(id, PinResult::Confirmed, Outcome::Confirmed, by)
            }
            (Kind::Confirm, Reply::Deny) => {
                self.resolve(id, PinResult::Denied, Outcome::Denied, by)
            }
            (_, Reply::Cancel) | (_, Reply::Deny) => {
                for gid in group_ids {
                    self.resolve(gid, PinResult::Cancelled, Outcome::Cancelled, by);
                }
            }
            (_, Reply::Forget) => {
                self.store.delete(&target.group);
                self.refresh_stored();
            }
            _ => return Err("that reply does not fit this kind of request".into()),
        }
        self.broadcast();
        warning.map_or(Ok(()), Err)
    }

    fn refresh_stored(&mut self) {
        let flags: Vec<(u64, bool)> = self
            .pending
            .values()
            .map(|p| {
                (
                    p.view.id,
                    self.storable(&p.view.prompt) && self.store.has(&p.view.group),
                )
            })
            .collect();
        for (id, stored) in flags {
            if let Some(p) = self.pending.get_mut(&id) {
                p.view.stored = stored;
            }
        }
    }

    fn withdraw_conn_quiet(&mut self, conn: u64) -> bool {
        let ids: Vec<u64> = self
            .pending
            .values()
            .filter(|p| p.conn == conn)
            .map(|p| p.view.id)
            .collect();
        for id in &ids {
            if let Some(p) = self.pending.remove(id) {
                self.record(p.view, Outcome::Withdrawn, "daemon");
            }
        }
        !ids.is_empty()
    }

    /// The requester hung up.
    pub fn withdraw_conn(&mut self, conn: u64) {
        if self.withdraw_conn_quiet(conn) {
            self.broadcast();
        }
    }

    pub fn expire(&mut self, now: Instant) {
        let due: Vec<u64> = self
            .pending
            .values()
            .filter(|p| p.deadline.is_some_and(|d| d <= now))
            .map(|p| p.view.id)
            .collect();
        if due.is_empty() {
            return;
        }
        for id in due {
            self.resolve(id, PinResult::TimedOut, Outcome::TimedOut, "daemon");
        }
        self.broadcast();
    }

    fn attach_ui(&mut self, conn: u64, notifies: bool, tx: Sender<ServerMsg>) {
        let _ = tx.send(self.snapshot());
        self.uis.insert(conn, Ui { notifies, tx });
    }

    fn detach_ui(&mut self, conn: u64) {
        self.uis.remove(&conn);
    }

    fn someone_notifies(&self) -> bool {
        self.uis.values().any(|u| u.notifies)
    }
}

fn notify(cfg: &Config, view: &RequestView) {
    let argv = cfg.notify_argv();
    let Some((prog, args)) = argv.split_first() else {
        return;
    };
    let title = format!("clortho: {} request", view.prompt.source.as_str());
    let body = format!("{} (from {})", view.prompt.summary(), view.origin.who());
    let args: Vec<String> = args
        .iter()
        .map(|a| a.replace("{title}", &title).replace("{body}", &body))
        .collect();
    let child = std::process::Command::new(prog)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if let Ok(mut child) = child {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

type Shared = Arc<Mutex<Broker>>;

fn serve(stream: UnixStream, conn: u64, shared: Shared) -> io::Result<()> {
    if !transport::peer_is_same_user(&stream) {
        return Err(io::Error::other("peer is another user"));
    }
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let Some(ClientMsg::Hello {
        version,
        role,
        name,
        notifies,
    }) = transport::recv(&mut reader)?
    else {
        return Err(io::Error::other("expected hello"));
    };
    if version != PROTOCOL_VERSION {
        let message =
            format!("protocol version {version} not supported (daemon speaks {PROTOCOL_VERSION})");
        transport::send(&mut writer, &ServerMsg::Error { message })?;
        return Ok(());
    }
    let policy = shared.lock().unwrap().cfg.store.as_str().to_string();
    transport::send(
        &mut writer,
        &ServerMsg::Welcome {
            version: PROTOCOL_VERSION,
            daemon_pid: std::process::id(),
            store: policy,
        },
    )?;

    // All later writes go through one channel so they cannot interleave.
    let (tx, rx) = mpsc::channel::<ServerMsg>();
    std::thread::spawn(move || {
        for msg in rx {
            if transport::send(&mut writer, &msg).is_err() {
                break;
            }
        }
    });

    match role {
        Role::Ui => {
            shared.lock().unwrap().attach_ui(conn, notifies, tx.clone());
            let result = loop {
                match transport::recv::<ClientMsg>(&mut reader) {
                    Ok(Some(ClientMsg::Answer { id, reply })) => {
                        let r = shared.lock().unwrap().answer(&name, id, reply);
                        if let Err(message) = r {
                            let _ = tx.send(ServerMsg::Error { message });
                        }
                    }
                    Ok(Some(_)) => {
                        let _ = tx.send(ServerMsg::Error {
                            message: "not valid for a ui client".into(),
                        });
                    }
                    Ok(None) => break Ok(()),
                    Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                        let _ = tx.send(ServerMsg::Error {
                            message: format!("bad message: {e}"),
                        });
                    }
                    Err(e) => break Err(e),
                }
            };
            shared.lock().unwrap().detach_ui(conn);
            result
        }
        Role::Requester => {
            let result = loop {
                match transport::recv::<ClientMsg>(&mut reader) {
                    Ok(Some(ClientMsg::Request { prompt, mut origin })) => {
                        // Process inspection happens outside the lock.
                        if let Some(pid) = origin.pid {
                            origin.chain = crate::provenance::chain(pid);
                        }
                        let (view, cfg, quiet) = {
                            let mut b = shared.lock().unwrap();
                            let view = b.submit(conn, prompt, origin, tx.clone());
                            (view, b.cfg.clone(), b.someone_notifies())
                        };
                        if let (Some(view), false) = (view, quiet) {
                            notify(&cfg, &view);
                        }
                    }
                    Ok(Some(_)) => {
                        let _ = tx.send(ServerMsg::Error {
                            message: "not valid for a requester".into(),
                        });
                    }
                    Ok(None) => break Ok(()),
                    Err(e) => break Err(e),
                }
            };
            shared.lock().unwrap().withdraw_conn(conn);
            result
        }
    }
}

pub fn run(cfg: Config) -> io::Result<()> {
    let path = transport::socket_path();
    transport::prepare_dir(&path)?;
    if UnixStream::connect(&path).is_ok() {
        eprintln!(
            "clortho: a daemon is already listening on {}",
            path.display()
        );
        return Ok(());
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    eprintln!(
        "clortho: listening on {} (store: {})",
        path.display(),
        cfg.store.as_str()
    );

    let store = store::open(&cfg);
    let shared: Shared = Arc::new(Mutex::new(Broker::new(cfg, store)));

    let ticker = shared.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(250));
            ticker.lock().unwrap().expire(Instant::now());
        }
    });

    let mut conn = 0u64;
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        conn += 1;
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = serve(stream, conn, shared);
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;
    use clortho_core::Secret;
    use std::sync::mpsc::Receiver;

    fn broker(policy: StorePolicy) -> Broker {
        let cfg = Config {
            store: policy,
            ..Config::default()
        };
        Broker::new(cfg, Box::new(MemoryStore::default()))
    }

    fn pin(key: Option<&str>) -> Prompt {
        let mut p = Prompt::new(Kind::Pin, Source::Ssh);
        p.key = key.map(String::from);
        p.allow_store = key.is_some();
        p
    }

    fn submit(b: &mut Broker, conn: u64, p: Prompt) -> (Option<RequestView>, Receiver<ServerMsg>) {
        let (tx, rx) = mpsc::channel();
        (b.submit(conn, p, Origin::default(), tx), rx)
    }

    fn secret_of(rx: &Receiver<ServerMsg>) -> Option<(String, bool)> {
        match rx.try_recv() {
            Ok(ServerMsg::Result {
                result: PinResult::Secret { secret, from_store },
            }) => Some((secret.expose().to_string(), from_store)),
            _ => None,
        }
    }

    #[test]
    fn one_answer_resolves_the_whole_group() {
        let mut b = broker(StorePolicy::Confirm);
        let (a, rx_a) = submit(&mut b, 1, pin(Some("id_ed25519")));
        let (_, rx_b) = submit(&mut b, 2, pin(Some("id_ed25519")));
        let (_, rx_c) = submit(&mut b, 3, pin(Some("other")));
        let reply = Reply::Secret {
            secret: Secret::new("pw"),
            remember: false,
        };
        b.answer("test", a.unwrap().id, reply).unwrap();
        assert_eq!(secret_of(&rx_a), Some(("pw".into(), false)));
        assert_eq!(secret_of(&rx_b), Some(("pw".into(), false)));
        assert!(secret_of(&rx_c).is_none());
        assert_eq!(b.pending.len(), 1);
        assert_eq!(b.recent.len(), 2);
    }

    #[test]
    fn unkeyed_requests_are_not_grouped() {
        let mut b = broker(StorePolicy::Confirm);
        let (a, rx_a) = submit(&mut b, 1, pin(None));
        let (_, rx_b) = submit(&mut b, 2, pin(None));
        b.answer(
            "test",
            a.unwrap().id,
            Reply::Secret {
                secret: Secret::new("pw"),
                remember: true,
            },
        )
        .unwrap();
        assert!(secret_of(&rx_a).is_some());
        assert!(secret_of(&rx_b).is_none());
    }

    #[test]
    fn confirm_policy_stores_then_releases_on_approval_only() {
        let mut b = broker(StorePolicy::Confirm);
        let (a, _rx) = submit(&mut b, 1, pin(Some("k")));
        b.answer(
            "test",
            a.unwrap().id,
            Reply::Secret {
                secret: Secret::new("pw"),
                remember: true,
            },
        )
        .unwrap();

        let (v, rx) = submit(&mut b, 2, pin(Some("k")));
        let v = v.expect("still needs a human under the confirm policy");
        assert!(v.stored);
        assert!(secret_of(&rx).is_none());
        b.answer("test", v.id, Reply::UseStored).unwrap();
        assert_eq!(secret_of(&rx), Some(("pw".into(), true)));
    }

    #[test]
    fn auto_policy_answers_from_store_and_bad_secret_is_dropped() {
        let mut b = broker(StorePolicy::Auto);
        let (a, _rx) = submit(&mut b, 1, pin(Some("k")));
        b.answer(
            "test",
            a.unwrap().id,
            Reply::Secret {
                secret: Secret::new("wrong"),
                remember: true,
            },
        )
        .unwrap();

        let (v, rx) = submit(&mut b, 2, pin(Some("k")));
        assert!(v.is_none());
        assert_eq!(secret_of(&rx), Some(("wrong".into(), true)));
        assert_eq!(b.recent[0].outcome, Outcome::FromStore);

        // The requester comes back saying it was wrong.
        let mut retry = pin(Some("k"));
        retry.error = Some("Bad passphrase".into());
        let (v, rx) = submit(&mut b, 3, retry);
        assert!(!v.expect("must be asked again").stored);
        assert!(secret_of(&rx).is_none());
    }

    #[test]
    fn off_policy_never_stores() {
        let mut b = broker(StorePolicy::Off);
        let (a, _rx) = submit(&mut b, 1, pin(Some("k")));
        b.answer(
            "test",
            a.unwrap().id,
            Reply::Secret {
                secret: Secret::new("pw"),
                remember: true,
            },
        )
        .unwrap();
        let (v, _rx) = submit(&mut b, 2, pin(Some("k")));
        let v = v.unwrap();
        assert!(!v.stored);
        assert!(b.answer("test", v.id, Reply::UseStored).is_err());
    }

    #[test]
    fn confirm_deny_cancel_and_mismatched_replies() {
        let mut b = broker(StorePolicy::Confirm);
        let (tx, rx) = mpsc::channel();
        let v = b
            .submit(
                1,
                Prompt::new(Kind::Confirm, Source::Ssh),
                Origin::default(),
                tx,
            )
            .unwrap();
        assert!(b.answer("t", v.id, Reply::UseStored).is_err());
        b.answer("t", v.id, Reply::Deny).unwrap();
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerMsg::Result {
                result: PinResult::Denied
            })
        ));

        let (v, rx) = submit(&mut b, 2, pin(None));
        let id = v.unwrap().id;
        assert!(b.answer("t", id, Reply::Confirm).is_err());
        b.answer("t", id, Reply::Cancel).unwrap();
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerMsg::Result {
                result: PinResult::Cancelled
            })
        ));
        assert!(b.answer("t", id, Reply::Cancel).is_err());
    }

    #[test]
    fn withdrawal_timeout_and_snapshots() {
        let mut b = broker(StorePolicy::Confirm);
        let (ui_tx, ui_rx) = mpsc::channel();
        b.attach_ui(99, false, ui_tx);
        assert!(
            matches!(ui_rx.try_recv(), Ok(ServerMsg::Snapshot { pending, .. }) if pending.is_empty())
        );

        let (_, _rx) = submit(&mut b, 1, pin(None));
        assert!(
            matches!(ui_rx.try_recv(), Ok(ServerMsg::Snapshot { pending, .. }) if pending.len() == 1)
        );
        b.withdraw_conn(1);
        assert!(b.pending.is_empty());
        assert_eq!(b.recent[0].outcome, Outcome::Withdrawn);

        let mut p = pin(None);
        p.timeout_secs = Some(5);
        let (_, rx) = submit(&mut b, 2, p);
        b.expire(Instant::now());
        assert_eq!(b.pending.len(), 1);
        b.expire(Instant::now() + Duration::from_secs(6));
        assert!(matches!(
            rx.try_recv(),
            Ok(ServerMsg::Result {
                result: PinResult::TimedOut
            })
        ));
        assert_eq!(b.recent[0].outcome, Outcome::TimedOut);
    }
}
