//! AF_UNIX stream sockets on every platform (native on Windows 10 1803+),
//! carrying newline-delimited JSON.

use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
pub use uds_windows::{UnixListener, UnixStream};

/// Socket location, in order: `$CLORTHO_SOCKET`; `$XDG_RUNTIME_DIR/clortho/clortho.sock`;
/// `~/.clortho/clortho.sock` (Unix) or `%LOCALAPPDATA%\clortho\clortho.sock` (Windows).
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("CLORTHO_SOCKET") {
        return PathBuf::from(p);
    }
    #[cfg(unix)]
    {
        if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
            return PathBuf::from(d).join("clortho").join("clortho.sock");
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".clortho")
            .join("clortho.sock")
    }
    #[cfg(windows)]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clortho")
            .join("clortho.sock")
    }
}

/// Create the socket's directory, private to the user.
pub fn prepare_dir(socket: &Path) -> io::Result<()> {
    let Some(dir) = socket.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let meta = std::fs::metadata(dir)?;
        if meta.uid() != unsafe { libc::geteuid() } {
            return Err(io::Error::other(format!(
                "{} is owned by another user",
                dir.display()
            )));
        }
        // Only tighten a directory we plausibly created; never chmod e.g. /tmp.
        if dir
            .file_name()
            .is_some_and(|n| n.to_string_lossy().contains("clortho"))
        {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// True if the peer runs as the same user as this process. On Windows the
/// socket's directory ACL (inside the user profile) is the only gate.
pub fn peer_is_same_user(stream: &UnixStream) -> bool {
    #[cfg(unix)]
    {
        match peer_uid(stream) {
            Ok(uid) => uid == unsafe { libc::geteuid() },
            Err(_) => false,
        }
    }
    #[cfg(windows)]
    {
        let _ = stream;
        true
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(cred.uid)
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    use std::os::fd::AsRawFd;
    let (mut uid, mut gid): (libc::uid_t, libc::gid_t) = (0, 0);
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}

pub fn send<T: Serialize>(w: &mut impl Write, msg: &T) -> io::Result<()> {
    let mut line = serde_json::to_vec(msg).map_err(io::Error::other)?;
    line.push(b'\n');
    let r = w.write_all(&line).and_then(|_| w.flush());
    zeroize::Zeroize::zeroize(&mut line);
    r
}

/// Read one message. `Ok(None)` on a clean end of stream.
pub fn recv<T: DeserializeOwned>(r: &mut impl BufRead) -> io::Result<Option<T>> {
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        if !line.trim().is_empty() {
            break;
        }
    }
    let parsed =
        serde_json::from_str(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
    zeroize::Zeroize::zeroize(&mut line);
    parsed.map(Some)
}
