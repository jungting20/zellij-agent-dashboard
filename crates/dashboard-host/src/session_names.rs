//! Resolve renamed Zellij sockets by their peer PID, never by pane IDs or titles.
use crate::process::Process;
use std::{collections::BTreeMap, path::Path};

#[derive(Default)]
pub struct SessionResolution {
    pub names: BTreeMap<u32, String>,
    pub displaced: std::collections::BTreeSet<u32>,
}

pub fn current_sessions(processes: &[Process]) -> SessionResolution {
    let sockets: BTreeMap<_, _> = processes
        .iter()
        .filter_map(|process| {
            crate::process::server_socket(process).map(|socket| (process.pid, socket))
        })
        .collect();
    let directories: std::collections::BTreeSet<_> = sockets
        .values()
        .filter_map(|socket| socket.parent().map(Path::to_owned))
        .collect();
    let mut resolution = SessionResolution::default();
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            use std::os::unix::fs::FileTypeExt;
            if !entry.file_type().is_ok_and(|kind| kind.is_socket()) {
                continue;
            }
            let Some(pid) = peer_pid(&entry.path()) else {
                continue;
            };
            if sockets.contains_key(&pid) {
                if let Some(name) = entry.file_name().to_str() {
                    resolution.names.insert(pid, name.to_owned());
                }
            }
        }
    }
    for (pid, socket) in &sockets {
        if resolution.names.contains_key(pid) {
            continue;
        }
        if let Some(peer) = peer_pid(socket) {
            if peer != *pid && resolution.names.contains_key(&peer) {
                resolution.displaced.insert(*pid);
            }
        }
    }
    resolution
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn peer_pid(path: &Path) -> Option<u32> {
    use std::{mem, os::fd::AsRawFd, os::unix::net::UnixStream};
    let stream = UnixStream::connect(path).ok()?;
    #[cfg(target_os = "macos")]
    let (level, option, mut value) = (libc::SOL_LOCAL, libc::LOCAL_PEERPID, 0 as libc::pid_t);
    #[cfg(target_os = "linux")]
    let (level, option, mut value) = (
        libc::SOL_SOCKET,
        libc::SO_PEERCRED,
        libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        },
    );
    let mut length = mem::size_of_val(&value) as libc::socklen_t;
    // SAFETY: value and length are initialized writable buffers of the exact
    // type expected by this platform's getsockopt; stream owns the live fd.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            level,
            option,
            std::ptr::from_mut(&mut value).cast(),
            &mut length,
        )
    };
    if result != 0 || length as usize != mem::size_of_val(&value) {
        return None;
    }
    #[cfg(target_os = "linux")]
    let value = value.pid;
    u32::try_from(value).ok().filter(|pid| *pid > 0)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn peer_pid(_path: &Path) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_server_whose_socket_is_owned_by_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("same session");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let pid = std::process::id();
        let processes: Vec<_> = [u32::MAX, pid]
            .into_iter()
            .map(|pid| Process {
                pid,
                ppid: 1,
                started: "now".into(),
                command: format!("zellij --server {}", socket.display()),
            })
            .collect();
        let resolution = current_sessions(&processes);
        assert_eq!(resolution.names.get(&pid).unwrap(), "same session");
        assert_eq!(resolution.displaced, [u32::MAX].into());
        drop(listener);
    }

    #[test]
    fn resolves_renamed_socket_to_its_server_process() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("old");
        let renamed = directory.path().join("새 세션");
        let listener = std::os::unix::net::UnixListener::bind(&original).unwrap();
        std::fs::rename(&original, &renamed).unwrap();
        let pid = std::process::id();
        let processes = vec![Process {
            pid,
            ppid: 1,
            started: "now".into(),
            command: format!("zellij --server {}", original.display()),
        }];
        assert_eq!(
            current_sessions(&processes).names.get(&pid).unwrap(),
            "새 세션"
        );
        drop(listener);
    }
}
