//! Resolve renamed Zellij sockets by their peer PID, never by pane IDs or titles.
use crate::process::Process;
use std::{collections::BTreeMap, path::Path};

pub fn current_sessions(processes: &[Process]) -> BTreeMap<u32, String> {
    let mut directories = std::collections::BTreeSet::new();
    let mut servers = std::collections::BTreeSet::new();
    for process in processes {
        let mut args = process.command.split_whitespace();
        let executable = args.next().unwrap_or("");
        if Path::new(executable).file_name().and_then(|s| s.to_str()) != Some("zellij")
            || args.next() != Some("--server")
        {
            continue;
        }
        servers.insert(process.pid);
        // Only the directory is needed; session names may contain spaces.
        if let Some(directory) = args.next().and_then(|socket| Path::new(socket).parent()) {
            directories.insert(directory.to_owned());
        }
    }
    let mut names = BTreeMap::new();
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
            if servers.contains(&pid) {
                if let Some(name) = entry.file_name().to_str() {
                    names.insert(pid, name.to_owned());
                }
            }
        }
    }
    names
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
        assert_eq!(current_sessions(&processes).get(&pid).unwrap(), "새 세션");
        drop(listener);
    }
}
