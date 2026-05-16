use std::path::Path;
use std::process::Command;

use synchrogit::clock;
use synchrogit::config::load_from_path;
use synchrogit::ipc::client;
use synchrogit::ipc::protocol::{Request, Response};
use synchrogit::ipc::server;
use synchrogit::runtime::Supervisor;
use synchrogit::state::SyncOutcome;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn ipc_status_sync_and_reload_roundtrip() {
    clock::init_local_offset();
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    init_repo(&repo);

    let config_path = tmp.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!(
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
        ),
    )
    .unwrap();

    let supervisor = Supervisor::spawn_loaded(load_from_path(&config_path).unwrap()).unwrap();
    let cancel = CancellationToken::new();
    let socket = tmp.path().join("synchrogit.sock");
    let ipc = server::spawn(socket.clone(), supervisor.control(), cancel.clone())
        .await
        .unwrap();

    let status = wait_for_status_cycle(&socket).await;
    match status {
        Response::Status { repos } => {
            assert_eq!(repos.len(), 1);
            assert_eq!(repos[0].name, "repo");
            assert_eq!(repos[0].path, repo.display().to_string());
            assert_eq!(repos[0].current_branch.as_deref(), Some("main"));
            assert!(repos[0].upstream.is_none());
            assert!(!repos[0].running);
            assert_eq!(repos[0].last_sync.last_outcome, SyncOutcome::NoOp);
            assert_eq!(repos[0].last_sync.consecutive_failures, 0);
            assert!(repos[0].last_sync.last_error.is_none());
            assert!(repos[0].last_sync.last_cycle_at.is_some());
        }
        other => panic!("unexpected status response: {other:?}"),
    }

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
            ok: true,
            message: "reloaded config: 0 added, 0 removed, 0 restarted, 1 unchanged".to_string(),
        }
    );

    cancel.cancel();
    let _ = ipc.join.await;
    supervisor.shutdown().await;
    assert!(!socket.exists(), "ipc socket should be cleaned up");
}

async fn wait_for_status_cycle(socket: &Path) -> Response {
    for _ in 0..50 {
        let response = client::request(socket, Request::Status).await.unwrap();
        if let Response::Status { repos } = &response
            && repos
                .first()
                .is_some_and(|repo| repo.last_sync.last_cycle_at.is_some())
        {
            return response;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    client::request(socket, Request::Status).await.unwrap()
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
