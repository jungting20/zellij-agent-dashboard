use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Finite, argv-based host commands. Drain both pipes while awaiting exit.
pub fn output(command: &mut Command, timeout: Duration, limit: usize) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("missing stdout")?;
    let stderr = child.stderr.take().ok_or("missing stderr")?;
    let read = move |pipe: Box<dyn Read + Send>| {
        let mut data = Vec::new();
        pipe.take(limit as u64 + 1)
            .read_to_end(&mut data)
            .map(|_| data)
    };
    let out = std::thread::spawn(move || read(Box::new(stdout)));
    let err = std::thread::spawn(move || read(Box::new(stderr)));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("host command timed out".into());
            }
        }
    };
    let stdout = out
        .join()
        .map_err(|_| "stdout reader failed")?
        .map_err(|e| e.to_string())?;
    let stderr = err
        .join()
        .map_err(|_| "stderr reader failed")?
        .map_err(|e| e.to_string())?;
    if stdout.len() > limit || stderr.len() > limit {
        return Err("host command output exceeds limit".into());
    }
    if !status.success() {
        return Err(format!(
            "host command failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    Ok(stdout)
}
