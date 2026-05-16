use std::env;
use std::path::PathBuf;

pub fn default_socket_path() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir).join("synchrogit.sock");
    }

    let uid = env::var("UID").unwrap_or_else(|_| "unknown".to_string());
    PathBuf::from(format!("/tmp/synchrogit-{uid}.sock"))
}
