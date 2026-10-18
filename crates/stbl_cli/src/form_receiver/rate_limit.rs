//! Bounded, process-safe state; no per-client files or directory scans.
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub(super) const MAX_GLOBAL_LIMIT: u32 = 10_000;
const MAX_STATE_BYTES: u64 = 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    minute: u64,
    total: u32,
    clients: BTreeMap<String, u32>,
}

pub(super) fn check(
    dir: &Path,
    identity: &str,
    limit: u32,
    global_limit: u32,
    now: u64,
) -> Result<bool> {
    let _lock = lock(dir)?;
    let minute = now / 60;
    let state_path = dir.join("rate-v2.json");
    let mut state = match private_options().read(true).open(&state_path) {
        Ok(file) => {
            if !file.metadata()?.is_file() {
                bail!("rate state is not a regular file");
            }
            let mut bytes = Vec::new();
            file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_STATE_BYTES {
                bail!("rate state exceeds size limit");
            }
            let state: State = serde_json::from_slice(&bytes)?;
            if state.clients.len() > MAX_GLOBAL_LIMIT as usize
                || state.total > MAX_GLOBAL_LIMIT
                || state
                    .clients
                    .values()
                    .any(|count| *count == 0 || *count > 1000)
                || state
                    .clients
                    .values()
                    .map(|count| u64::from(*count))
                    .sum::<u64>()
                    != u64::from(state.total)
                || state
                    .clients
                    .keys()
                    .any(|key| key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                bail!("invalid rate state");
            }
            state
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => State {
            minute,
            total: 0,
            clients: BTreeMap::new(),
        },
        Err(error) => return Err(error.into()),
    };
    if state.minute != minute {
        state = State {
            minute,
            total: 0,
            clients: BTreeMap::new(),
        };
    }
    let key = blake3::hash(identity.as_bytes()).to_hex().to_string();
    if state.total >= global_limit || state.clients.get(&key).copied().unwrap_or(0) >= limit {
        return Ok(false);
    }
    state.total += 1;
    *state.clients.entry(key).or_default() += 1;
    let bytes = serde_json::to_vec(&state)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        bail!("rate state exceeds size limit");
    }
    // A crash leaves either the old or new complete state. The pending path is fixed,
    // so an interrupted update cannot accumulate temporary files.
    let pending = dir.join("rate-v2.pending");
    let mut file = private_options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&pending)?;
    if !file.metadata()?.is_file() {
        bail!("pending rate state is not a regular file");
    }
    file.write_all(&bytes)?;
    drop(file);
    std::fs::rename(&pending, &state_path)?;
    Ok(true)
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    options
}

#[cfg(unix)]
fn lock(dir: &Path) -> Result<File> {
    use std::os::fd::AsRawFd;
    let file = private_options()
        .read(true)
        .write(true)
        .create(true)
        .open(dir.join("rate-v2.lock"))?;
    if !file.metadata()?.is_file() {
        bail!("rate lock is not a regular file");
    }
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        // SAFETY: file owns this valid descriptor for the complete lock lifetime.
        // Closing it (including process death) releases the kernel lock.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(file);
        }
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ) {
            return Err(error.into());
        }
        if Instant::now() >= deadline {
            bail!("rate state lock timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(not(unix))]
fn lock(_dir: &Path) -> Result<File> {
    bail!("form receiver rate storage requires a Unix host")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn caps_global_clients_expires_counts_and_never_scans_legacy_tokens() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("stbl-rate-0-legacy-0");
        std::fs::write(&old, "legacy").unwrap();
        assert!(check(temp.path(), "one", 5, 2, 60).unwrap());
        assert!(check(temp.path(), "two", 5, 2, 60).unwrap());
        for identity in 0..100 {
            assert!(!check(temp.path(), &format!("new-{identity}"), 5, 2, 60).unwrap());
        }
        let state: State =
            serde_json::from_slice(&std::fs::read(temp.path().join("rate-v2.json")).unwrap())
                .unwrap();
        assert_eq!(state.clients.len(), 2);
        assert_eq!(state.total, 2);
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 3);
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "legacy");
        assert!(check(temp.path(), "one", 5, 2, 120).unwrap());
        let state: State =
            serde_json::from_slice(&std::fs::read(temp.path().join("rate-v2.json")).unwrap())
                .unwrap();
        assert_eq!(state.clients.len(), 1);
        assert_eq!(state.total, 1);
    }

    #[test]
    fn concurrent_requests_cannot_exceed_per_client_limit() {
        let temp = tempfile::tempdir().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(20));
        let mut workers = Vec::new();
        for _ in 0..20 {
            let dir = temp.path().to_path_buf();
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                check(&dir, "same-client", 3, 10, 60).unwrap()
            }));
        }
        let accepted = workers
            .into_iter()
            .map(|worker| usize::from(worker.join().unwrap()))
            .sum::<usize>();
        assert_eq!(accepted, 3);
    }

    #[test]
    fn invalid_or_oversized_state_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("rate-v2.json");
        std::fs::write(&state, "not json").unwrap();
        assert!(check(temp.path(), "one", 5, 1000, 60).is_err());
        assert_eq!(std::fs::read_to_string(&state).unwrap(), "not json");
        std::fs::write(&state, vec![b' '; MAX_STATE_BYTES as usize + 1]).unwrap();
        assert!(check(temp.path(), "one", 5, 1000, 60).is_err());
    }

    #[test]
    fn storage_symlinks_are_rejected_without_touching_target() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        std::fs::write(&target, "keep").unwrap();
        for name in ["rate-v2.lock", "rate-v2.json", "rate-v2.pending"] {
            let link = temp.path().join(name);
            symlink(&target, &link).unwrap();
            assert!(check(temp.path(), "one", 5, 1000, 60).is_err());
            std::fs::remove_file(link).unwrap();
            assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep");
        }
    }
}
