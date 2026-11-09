//! Remember automatic opening for this login, including across panel restarts.
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};

pub const RUNTIME_DIRECTORY: &str = "cosmic-applet-finance-dashboard";
const PREVIOUS_RUNTIME_DIRECTORY: &str = "daily-work-cosmic-applet";

pub fn claim_current_session() -> Result<bool> {
    let runtime = dirs::runtime_dir().context("XDG_RUNTIME_DIR is unavailable")?;
    let session = std::env::var("XDG_SESSION_ID")
        .ok()
        .filter(|id| !id.is_empty())
        .context("XDG_SESSION_ID is unavailable")?;
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    claim_session(&runtime, boot.trim(), &session)
}

fn runtime_directory(runtime: &Path) -> std::path::PathBuf {
    let current = runtime.join(RUNTIME_DIRECTORY);
    let previous = runtime.join(PREVIOUS_RUNTIME_DIRECTORY);
    if !current.exists() && previous.is_dir() {
        let _ = fs::rename(&previous, &current);
    }
    current
}

fn claim_session(runtime: &Path, boot: &str, session: &str) -> Result<bool> {
    let directory = runtime_directory(runtime);
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    // Hash the identifiers so they cannot become path components. The boot ID
    // prevents reused logind session numbers from matching an earlier login.
    let key = Sha256::digest(format!("{boot}\0{session}"));
    let path = directory.join(format!("login-{key:x}.shown"));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_restarts_share_a_marker_but_new_logins_can_open() {
        let root =
            std::env::temp_dir().join(format!("finance-dashboard-login-{}", rand::random::<u64>()));
        assert!(claim_session(&root, "boot-a", "3").unwrap());
        assert!(!claim_session(&root, "boot-a", "3").unwrap());
        assert!(claim_session(&root, "boot-a", "4").unwrap());
        assert!(claim_session(&root, "boot-b", "3").unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simultaneous_panel_instances_only_claim_once() {
        let root =
            std::env::temp_dir().join(format!("finance-dashboard-login-{}", rand::random::<u64>()));
        let barrier = std::sync::Barrier::new(4);
        let claims = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        claim_session(&root, "boot", "3").unwrap()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(|claimed| *claimed)
                .count()
        });
        assert_eq!(claims, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn previous_runtime_directory_is_adopted() {
        let root =
            std::env::temp_dir().join(format!("finance-dashboard-login-{}", rand::random::<u64>()));
        let previous = root.join(PREVIOUS_RUNTIME_DIRECTORY);
        fs::create_dir_all(&previous).unwrap();
        fs::write(previous.join("login-existing.shown"), b"").unwrap();
        let current = runtime_directory(&root);
        assert_eq!(current, root.join(RUNTIME_DIRECTORY));
        assert!(current.join("login-existing.shown").is_file());
        assert!(!previous.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
