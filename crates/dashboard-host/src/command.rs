use std::{
    ffi::OsString,
    fmt,
    io::Read,
    os::unix::{io::AsRawFd, process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(OsString, Option<OsString>)>,
    pub timeout: Option<Duration>,
    pub limit: usize,
    pub capture: bool,
}

impl CommandSpec {
    pub fn new(program: impl Into<OsString>, timeout: Option<Duration>, limit: usize) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            timeout,
            limit,
            capture: true,
        }
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }
}

pub struct CommandOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub enum CommandError {
    Execution(String),
    Timeout,
    OutputLimit,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execution(e) => write!(f, "host command execution failed: {e}"),
            Self::Timeout => write!(f, "host command timed out"),
            Self::OutputLimit => write!(f, "host command output exceeds limit"),
        }
    }
}

pub trait CommandRunner {
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, CommandError>;
}

pub struct SystemCommandRunner;

enum ReadEvent {
    Finished(bool, Vec<u8>),
    Error(CommandError),
}

fn drain(
    mut pipe: impl Read + AsRawFd,
    stdout: bool,
    limit: usize,
    sender: mpsc::Sender<ReadEvent>,
    stop: Arc<AtomicBool>,
) {
    // SAFETY: pipe owns a live descriptor; F_GETFL/F_SETFL only change its flags.
    let flags = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        let _ = sender.send(ReadEvent::Error(CommandError::Execution(
            std::io::Error::last_os_error().to_string(),
        )));
        return;
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        if stop.load(Ordering::Acquire) {
            return;
        }
        match pipe.read(&mut buffer) {
            Ok(0) => {
                let _ = sender.send(ReadEvent::Finished(stdout, bytes));
                return;
            }
            Ok(n) if n > limit.saturating_sub(bytes.len()) => {
                let _ = sender.send(ReadEvent::Error(CommandError::OutputLimit));
                return;
            }
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => {
                let _ = sender.send(ReadEvent::Error(CommandError::Execution(e.to_string())));
                return;
            }
        }
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, spec: &CommandSpec) -> Result<CommandOutput, CommandError> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdin(Stdio::null())
            .process_group(0);
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        for (key, value) in &spec.env {
            match value {
                Some(value) => {
                    command.env(key, value);
                }
                None => {
                    command.env_remove(key);
                }
            }
        }
        if spec.capture {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
        } else {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        let mut child = command
            .spawn()
            .map_err(|e| CommandError::Execution(e.to_string()))?;
        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let mut readers = Vec::new();
        if spec.capture {
            let stdout = child.stdout.take().expect("piped stdout");
            let stderr = child.stderr.take().expect("piped stderr");
            let limit = spec.limit;
            let out_sender = sender.clone();
            let out_stop = stop.clone();
            readers.push(std::thread::spawn(move || {
                drain(stdout, true, limit, out_sender, out_stop)
            }));
            let err_stop = stop.clone();
            readers.push(std::thread::spawn(move || {
                drain(stderr, false, limit, sender, err_stop)
            }));
        }
        let started = Instant::now();
        let result = (|| {
            let mut stdout = None;
            let mut stderr = None;
            loop {
                if spec.capture {
                    while let Ok(event) = receiver.try_recv() {
                        match event {
                            ReadEvent::Finished(true, data) => stdout = Some(data),
                            ReadEvent::Finished(false, data) => stderr = Some(data),
                            ReadEvent::Error(error) => return Err(error),
                        }
                    }
                }
                if let Some(status) = child
                    .try_wait()
                    .map_err(|e| CommandError::Execution(e.to_string()))?
                {
                    if !spec.capture || (stdout.is_some() && stderr.is_some()) {
                        return Ok(CommandOutput {
                            success: status.success(),
                            code: status.code(),
                            stdout: stdout.unwrap_or_default(),
                            stderr: stderr.unwrap_or_default(),
                        });
                    }
                }
                if spec
                    .timeout
                    .is_some_and(|timeout| started.elapsed() >= timeout)
                {
                    return Err(CommandError::Timeout);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })();
        if result.is_err() {
            // SAFETY: the child was spawned into its own process group. A
            // negative PID targets only that group, including inherited pipes.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        // A descendant can escape the group and retain a pipe. Cancellation
        // still closes our descriptors and releases buffers before returning.
        stop.store(true, Ordering::Release);
        for reader in readers {
            let _ = reader.join();
        }
        result
    }
}

pub fn output(runner: &dyn CommandRunner, spec: CommandSpec) -> Result<Vec<u8>, String> {
    let output = runner.run(&spec).map_err(|e| e.to_string())?;
    if !output.success {
        return Err(format!(
            "host command failed (exit {:?}): {}",
            output.code,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_both_streams_and_nonzero_exit() {
        let spec = CommandSpec::new("/bin/sh", Some(Duration::from_secs(2)), 1024)
            .args(["-c", "printf hello; printf problem >&2; exit 7"]);
        let result = SystemCommandRunner.run(&spec).unwrap();
        assert!(!result.success);
        assert_eq!(result.code, Some(7));
        assert_eq!(result.stdout, b"hello");
        assert_eq!(result.stderr, b"problem");
    }

    #[test]
    fn timeout_and_output_limit_stop_children() {
        let spec =
            CommandSpec::new("/bin/sleep", Some(Duration::from_millis(30)), 1024).args(["5"]);
        assert!(matches!(
            SystemCommandRunner.run(&spec),
            Err(CommandError::Timeout)
        ));
        let spec = CommandSpec::new("/usr/bin/yes", Some(Duration::from_secs(2)), 32);
        assert!(matches!(
            SystemCommandRunner.run(&spec),
            Err(CommandError::OutputLimit)
        ));
    }

    #[test]
    fn timeout_and_output_limit_stop_descendants_that_inherit_pipes() {
        for output_limit in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let marker = directory.path().join("survived");
            let script = if output_limit {
                "(sleep 0.3; printf survived > \"$1\") & exec /usr/bin/yes"
            } else {
                "(sleep 0.3; printf survived > \"$1\") & wait"
            };
            let spec = CommandSpec::new("/bin/sh", Some(Duration::from_millis(100)), 32).args([
                std::ffi::OsStr::new("-c"),
                std::ffi::OsStr::new(script),
                std::ffi::OsStr::new("audit"),
                marker.as_os_str(),
            ]);
            let result = SystemCommandRunner.run(&spec);
            assert!(matches!(
                (&result, output_limit),
                (Err(CommandError::OutputLimit), true) | (Err(CommandError::Timeout), false)
            ));
            std::thread::sleep(Duration::from_millis(400));
            assert!(!marker.exists(), "descendant survived command cancellation");
        }
    }

    #[test]
    fn cancellation_releases_reader_even_when_writer_keeps_pipe_open() {
        use std::{io::Write, os::unix::net::UnixStream};
        let (reader, mut writer) = UnixStream::pair().unwrap();
        writer
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        writer
            .write_all(b"buffer retained until cancellation")
            .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let reader_stop = stop.clone();
        let (sender, receiver) = mpsc::channel();
        let (done, completed) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            drain(reader, true, 1024, sender, reader_stop);
            done.send(()).unwrap();
        });
        std::thread::sleep(Duration::from_millis(30));
        stop.store(true, Ordering::Release);
        completed.recv_timeout(Duration::from_secs(1)).unwrap();
        handle.join().unwrap();
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        match writer.read(&mut [0; 1]) {
            Ok(0) => {}
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
            other => panic!("reader did not close: {other:?}"),
        }
    }

    #[test]
    fn argv_is_literal_and_spawn_errors_are_distinct() {
        let value = "$(literal) `text` 한글\nsecond line";
        let spec = CommandSpec::new("/usr/bin/printf", Some(Duration::from_secs(2)), 1024)
            .args(["%s", value]);
        assert_eq!(
            output(&SystemCommandRunner, spec).unwrap(),
            value.as_bytes()
        );
        let directory = tempfile::tempdir().unwrap();
        let spec = CommandSpec::new(
            directory.path().join("missing-executable").into_os_string(),
            None,
            0,
        );
        assert!(matches!(
            SystemCommandRunner.run(&spec),
            Err(CommandError::Execution(_))
        ));
    }

    #[test]
    fn cwd_environment_and_discard_mode() {
        let directory = tempfile::tempdir().unwrap();
        let mut spec = CommandSpec::new("/bin/sh", Some(Duration::from_secs(2)), 1024).args([
            "-c",
            "printf '%s' \"$ZAD_TEST\"; test -z \"$PATH\"; test -z \"$ZAD_REMOVE\"; test \"$PWD\" = \"$ZAD_CWD\"",
        ]);
        spec.cwd = Some(directory.path().to_owned());
        spec.env = vec![
            ("ZAD_TEST".into(), Some("한글 값".into())),
            ("PATH".into(), Some("".into())),
            ("ZAD_REMOVE".into(), Some("remove me".into())),
            ("ZAD_REMOVE".into(), None),
            (
                "ZAD_CWD".into(),
                Some(
                    std::fs::canonicalize(directory.path())
                        .unwrap()
                        .into_os_string(),
                ),
            ),
        ];
        assert_eq!(
            output(&SystemCommandRunner, spec.clone()).unwrap(),
            "한글 값".as_bytes()
        );
        spec.capture = false;
        assert!(output(&SystemCommandRunner, spec).unwrap().is_empty());
    }
}
