//! Time-bounded local MTA submission, including pipe writes and child cleanup.
use std::path::Path;
use std::time::Duration;

use anyhow::Result;

#[derive(Debug)]
pub(super) struct DeliveryTimeout;
impl std::fmt::Display for DeliveryTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("sendmail deadline exceeded; delivery is uncertain")
    }
}
impl std::error::Error for DeliveryTimeout {}

#[cfg(unix)]
pub(super) fn send(
    path: &Path,
    sender: &str,
    recipient: &str,
    message: &[u8],
    timeout: Duration,
) -> Result<()> {
    use anyhow::{Context, bail};
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::Instant;

    struct Submission(Child, bool);
    impl Drop for Submission {
        fn drop(&mut self) {
            if !self.1 {
                if let Ok(pid) = i32::try_from(self.0.id()) {
                    // SAFETY: process_group(0) gave this unreaped child its own group.
                    // Its PID cannot be reused until wait reaps it. Signal that group
                    // so helper children cannot keep the pipe/worker occupied.
                    unsafe {
                        libc::kill(-pid, libc::SIGKILL);
                    }
                }
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    let deadline = Instant::now() + timeout;
    let child = Command::new(path)
        .args(["-i", "-f", sender, "--", recipient])
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut submission = Submission(child, false);
    let mut input = submission
        .0
        .stdin
        .take()
        .context("missing sendmail stdin")?;
    // SAFETY: input owns this valid pipe descriptor; F_GETFL/F_SETFL only change
    // its I/O flags. Nonblocking writes let the same deadline cover a full pipe.
    let flags = unsafe { libc::fcntl(input.as_raw_fd(), libc::F_GETFL) };
    if flags == -1
        || unsafe { libc::fcntl(input.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut remaining = message;
    while !remaining.is_empty() {
        if Instant::now() >= deadline {
            return Err(DeliveryTimeout.into());
        }
        match input.write(remaining) {
            Ok(0) => bail!("sendmail pipe closed before message was written"),
            Ok(count) => remaining = &remaining[count..],
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    drop(input);
    loop {
        if let Some(status) = submission.0.try_wait()? {
            submission.1 = true;
            if !status.success() {
                bail!("sendmail did not queue the message");
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(DeliveryTimeout.into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(not(unix))]
pub(super) fn send(
    _path: &Path,
    _sender: &str,
    _recipient: &str,
    _message: &[u8],
    _timeout: Duration,
) -> Result<()> {
    anyhow::bail!("form receiver delivery requires a Unix host")
}
