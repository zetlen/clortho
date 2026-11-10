//! Client side of the daemon connection, shared by the shims and the UIs.

use clortho_core::proto::*;
use clortho_core::transport::{self, UnixStream};
use std::io::{self, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

pub struct Conn {
    pub reader: BufReader<UnixStream>,
    pub writer: UnixStream,
    pub daemon_pid: u32,
    pub store_policy: String,
}

pub fn connect(role: Role, name: &str, notifies: bool) -> io::Result<Conn> {
    let stream = UnixStream::connect(transport::socket_path())?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let hello = ClientMsg::Hello {
        version: PROTOCOL_VERSION,
        role,
        name: name.to_string(),
        notifies,
    };
    transport::send(&mut writer, &hello)?;
    match transport::recv::<ServerMsg>(&mut reader)? {
        Some(ServerMsg::Welcome {
            daemon_pid, store, ..
        }) => Ok(Conn {
            reader,
            writer,
            daemon_pid,
            store_policy: store,
        }),
        Some(ServerMsg::Error { message }) => Err(io::Error::other(message)),
        _ => Err(io::Error::other("unexpected reply to hello")),
    }
}

/// Connect, starting a detached daemon first if none is listening.
/// `CLORTHO_NO_AUTOSTART=1` disables the spawn.
pub fn connect_or_spawn(role: Role, name: &str, notifies: bool) -> io::Result<Conn> {
    if let Ok(c) = connect(role, name, notifies) {
        return Ok(c);
    }
    if std::env::var_os("CLORTHO_NO_AUTOSTART").is_some() {
        return Err(io::Error::other("daemon not running"));
    }
    spawn_daemon()?;
    let mut last = io::Error::other("daemon did not start");
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(50));
        match connect(role, name, notifies) {
            Ok(c) => return Ok(c),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn daemon_program() -> std::path::PathBuf {
    let exe_name = if cfg!(windows) {
        "clortho.exe"
    } else {
        "clortho"
    };
    if let Ok(me) = std::env::current_exe()
        && let Some(sibling) = me.parent().map(|d| d.join(exe_name))
        && sibling.exists()
    {
        return sibling;
    }
    exe_name.into()
}

fn spawn_daemon() -> io::Result<()> {
    let mut cmd = Command::new(daemon_program());
    cmd.arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own session: the daemon must outlive the gpg-agent or shell that
        // happened to trigger it.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().map(drop)
}

impl Conn {
    pub fn send(&mut self, msg: &ClientMsg) -> io::Result<()> {
        transport::send(&mut self.writer, msg)
    }

    pub fn recv(&mut self) -> io::Result<Option<ServerMsg>> {
        transport::recv(&mut self.reader)
    }

    /// Requester: submit a prompt and block until it is resolved.
    pub fn request(&mut self, prompt: Prompt, origin: Origin) -> io::Result<PinResult> {
        self.send(&ClientMsg::Request { prompt, origin })?;
        loop {
            match self.recv()? {
                Some(ServerMsg::Result { result }) => return Ok(result),
                Some(ServerMsg::Error { message }) => return Err(io::Error::other(message)),
                Some(_) => continue,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "daemon went away",
                    ));
                }
            }
        }
    }

    /// UI: wait for the next snapshot.
    pub fn snapshot(&mut self) -> io::Result<(Vec<RequestView>, Vec<HistoryEntry>)> {
        loop {
            match self.recv()? {
                Some(ServerMsg::Snapshot { pending, recent }) => return Ok((pending, recent)),
                Some(_) => continue,
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "daemon went away",
                    ));
                }
            }
        }
    }
}
