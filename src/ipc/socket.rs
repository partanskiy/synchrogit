use std::env;
use std::path::PathBuf;

#[cfg(unix)]
pub fn default_socket_path() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir).join("synchrogit.sock");
    }

    tmp_socket_path()
}

// Where a control client looks for a running daemon: the user-session default
// first, then the path the packaged `synchrogit@.service` system template
// binds, then the sessionless /tmp fallback. Returns the first candidate with
// a live socket, or the bind default so a connection error points at a
// sensible path.
#[cfg(unix)]
pub fn discover_socket_path() -> PathBuf {
    let mut candidates = vec![default_socket_path()];
    if let Some(user) = env::var_os("USER").filter(|v| !v.is_empty()) {
        candidates.push(
            PathBuf::from("/run/synchrogit")
                .join(user)
                .join("synchrogit.sock"),
        );
    }
    candidates.push(tmp_socket_path());

    first_existing(&candidates).unwrap_or_else(|| candidates[0].clone())
}

#[cfg(unix)]
fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.exists()).cloned()
}

#[cfg(unix)]
fn tmp_socket_path() -> PathBuf {
    // SAFETY: geteuid never fails and touches no memory.
    let uid = unsafe { libc::geteuid() };
    PathBuf::from(format!("/tmp/synchrogit-{uid}.sock"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::first_existing;

    #[test]
    fn picks_first_candidate_that_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing.sock");
        let present = tmp.path().join("present.sock");
        std::fs::write(&present, b"").unwrap();

        let candidates = vec![missing.clone(), present.clone()];
        assert_eq!(first_existing(&candidates), Some(present));
        assert_eq!(first_existing(&[missing]), None);
    }
}

#[cfg(windows)]
pub fn default_socket_path() -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    env::var_os("USERPROFILE").hash(&mut hash);
    PathBuf::from(format!(r"\\.\pipe\synchrogit-{:016x}", hash.finish()))
}
#[cfg(windows)]
pub fn discover_socket_path() -> PathBuf {
    default_socket_path()
}
