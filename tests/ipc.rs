use std::path::Path;
use std::process::Command;

use synchrogit::clock;
use synchrogit::config::parse_str;
use synchrogit::ipc::client;
use synchrogit::ipc::protocol::{Request, Response};
use synchrogit::ipc::server;
use synchrogit::runtime::Supervisor;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn ipc_status_sync_and_reload_roundtrip() {
    clock::init_local_offset();
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    init_repo(&repo);

    let config = parse_str(&format!(
        r#"
[defaults]
interval = "1h"
debounce = "10ms"
auto-pull = false
auto-push = false

[[repo]]
name = "repo"
path = "{}"
"#,
        toml_path(&repo),
    ))
    .unwrap();

    let supervisor = Supervisor::spawn(config).unwrap();
    let cancel = CancellationToken::new();
    let socket = tmp.path().join("synchrogit.sock");
    let ipc = server::spawn(socket.clone(), supervisor.control(), cancel.clone())
        .await
        .unwrap();

    assert_eq!(
        client::request(&socket, Request::Status).await.unwrap(),
        Response::Status {
            repos: vec![synchrogit::ipc::protocol::RepoStatus {
                name: "repo".to_string(),
                path: repo.display().to_string(),
            }],
        }
    );

    assert_eq!(
        client::request(&socket, Request::Sync { repo: None })
            .await
            .unwrap(),
        Response::Synced {
            queued: vec!["repo".to_string()],
        }
    );

    assert_eq!(
        client::request(
            &socket,
            Request::Sync {
                repo: Some("missing".to_string()),
            },
        )
        .await
        .unwrap(),
        Response::Error {
            message: "unknown repo `missing`".to_string(),
        }
    );

    assert_eq!(
        client::request(&socket, Request::Reload).await.unwrap(),
        Response::Reloaded {
            ok: false,
            message: "manual reload lands with the config hot-reload milestone".to_string(),
        }
    );

    cancel.cancel();
    let _ = ipc.join.await;
    supervisor.shutdown().await;
    assert!(!socket.exists(), "ipc socket should be cleaned up");
}

fn init_repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    run_git(path, &["init", "-q", "-b", "main"]);
    run_git(path, &["config", "user.email", "test@example.com"]);
    run_git(path, &["config", "user.name", "Test User"]);
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to spawn git");
    if !output.status.success() {
        panic!(
            "git {args:?} in {dir:?} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}
