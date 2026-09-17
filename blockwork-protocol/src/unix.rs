use super::{Connection, NAME};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tokio::net::{UnixListener, UnixStream};

pub fn socket_path() -> PathBuf {
    let dir = match (std::env::var_os("FLATPAK_ID"), dirs::runtime_dir()) {
        (Some(app_id), Some(runtime)) => runtime.join("app").join(app_id),
        (None, Some(runtime)) => runtime.join("blockwork"),
        _ => std::env::temp_dir().join(format!("blockwork-{}", uid())),
    };
    dir.join(format!("{NAME}.sock"))
}

fn uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn denied() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "untrusted IPC endpoint")
}

fn validate_dir(dir: &Path) -> io::Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(dir)?;
    let meta = file.metadata()?;
    if meta.uid() != uid() || meta.mode() & 0o077 != 0 {
        return Err(denied());
    }
    // A writable ancestor could let another user replace the directory.
    for parent in dir.ancestors().skip(1) {
        if parent.as_os_str().is_empty() {
            continue;
        }
        let m = parent.symlink_metadata()?;
        if m.file_type().is_symlink() && m.uid() != 0 {
            return Err(denied());
        }
        if m.uid() != 0 && m.uid() != uid() {
            return Err(denied());
        }
        if !m.file_type().is_symlink()
            && m.mode() & 0o022 != 0
            && !(m.uid() == 0 && m.mode() & (libc::S_ISVTX as u32) != 0)
        {
            return Err(denied());
        }
    }
    let canonical = dir.canonicalize()?;
    for parent in canonical.ancestors().skip(1) {
        let m = parent.metadata()?;
        if m.uid() != 0 && m.uid() != uid() {
            return Err(denied());
        }
        if m.mode() & 0o022 != 0 && !(m.uid() == 0 && m.mode() & (libc::S_ISVTX as u32) != 0) {
            return Err(denied());
        }
    }
    Ok(())
}

fn validate_socket(path: &Path) -> io::Result<std::fs::Metadata> {
    let m = path.symlink_metadata()?;
    if !m.file_type().is_socket() || m.uid() != uid() || m.mode() & 0o077 != 0 {
        return Err(denied());
    }
    Ok(m)
}

fn check_peer(stream: &UnixStream) -> io::Result<()> {
    if stream.peer_cred()?.uid() != uid() {
        return Err(denied());
    }
    Ok(())
}

pub async fn connect() -> io::Result<Connection> {
    let path = socket_path();
    validate_dir(path.parent().ok_or_else(denied)?)?;
    validate_socket(&path)?;
    let stream = tokio::time::timeout(super::CONNECT_TIMEOUT, UnixStream::connect(path)).await??;
    check_peer(&stream)?;
    Ok(Box::new(stream))
}

pub struct Listener {
    listener: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
    _lock: File,
}

impl Listener {
    pub async fn bind() -> io::Result<Self> {
        Self::bind_path(socket_path()).await
    }

    async fn bind_path(path: PathBuf) -> io::Result<Self> {
        let dir = path.parent().ok_or_else(denied)?;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        validate_dir(dir)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path.with_extension("lock"))?;
        let m = lock.metadata()?;
        if !m.is_file() || m.uid() != uid() || m.mode() & 0o077 != 0 {
            return Err(denied());
        }
        // Keep the lock file in place so competing launches lock the same inode.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let e = io::Error::last_os_error();
            return Err(if e.kind() == io::ErrorKind::WouldBlock {
                io::Error::new(io::ErrorKind::AddrInUse, e)
            } else {
                e
            });
        }
        match validate_socket(&path) {
            Ok(_) => match tokio::time::timeout(super::CONNECT_TIMEOUT, UnixStream::connect(&path))
                .await?
            {
                Ok(_) => return Err(io::ErrorKind::AddrInUse.into()),
                Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(&path)?
                }
                Err(e) => return Err(e),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        // Publish only after permissions are set.
        let staging = path.with_file_name(format!(".{NAME}"));
        match staging.symlink_metadata() {
            Ok(m) if m.file_type().is_socket() && m.uid() == uid() => {
                std::fs::remove_file(&staging)?
            }
            Ok(_) => return Err(denied()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let listener = UnixListener::bind(&staging)?;
        std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o600))?;
        std::fs::rename(&staging, &path)?;
        let m = validate_socket(&path)?;
        Ok(Self {
            listener,
            path,
            identity: (m.dev(), m.ino()),
            _lock: lock,
        })
    }

    pub async fn accept(&self) -> io::Result<Connection> {
        let (stream, _) = self.listener.accept().await?;
        check_peer(&stream)?;
        Ok(Box::new(stream))
    }
}

impl Listener {
    pub fn cleanup(&self) {
        if self
            .path
            .symlink_metadata()
            .is_ok_and(|m| (m.dev(), m.ino()) == self.identity)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "blockwork-ipc-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> PathBuf {
            self.0.join("daemon.sock")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn rejects_public_directory_and_symlink() {
        let tmp = Temp::new();
        std::fs::set_permissions(&tmp.0, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(
            Listener::bind_path(tmp.path()).await.err().unwrap().kind(),
            io::ErrorKind::PermissionDenied
        );
        std::fs::set_permissions(&tmp.0, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = tmp.0.join("link");
        std::os::unix::fs::symlink(&tmp.0, &link).unwrap();
        assert!(Listener::bind_path(link.join("daemon.sock")).await.is_err());
    }

    #[tokio::test]
    async fn preserves_non_socket_files() {
        let tmp = Temp::new();
        std::fs::write(tmp.path(), "keep").unwrap();
        assert!(Listener::bind_path(tmp.path()).await.is_err());
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "keep");
    }

    #[tokio::test]
    async fn stale_recovery_and_exclusive_owner() {
        let tmp = Temp::new();
        let stale = std::os::unix::net::UnixListener::bind(tmp.path()).unwrap();
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600)).unwrap();
        drop(stale);
        let listener = Listener::bind_path(tmp.path()).await.unwrap();
        assert_eq!(
            Listener::bind_path(tmp.path()).await.err().unwrap().kind(),
            io::ErrorKind::AddrInUse
        );
        let client = UnixStream::connect(tmp.path()).await.unwrap();
        check_peer(&client).unwrap();
        let _server = listener.accept().await.unwrap();
        assert_eq!(tmp.path().metadata().unwrap().mode() & 0o777, 0o600);
        drop(listener);
        assert!(!tmp.path().exists());
        assert!(Listener::bind_path(tmp.path()).await.is_ok());
    }

    #[tokio::test]
    async fn drop_preserves_replaced_endpoint() {
        let tmp = Temp::new();
        let listener = Listener::bind_path(tmp.path()).await.unwrap();
        std::fs::remove_file(tmp.path()).unwrap();
        std::fs::write(tmp.path(), "replacement").unwrap();
        drop(listener);
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "replacement");
    }
}
