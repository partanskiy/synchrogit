use std::path::Path;
use std::process::Command;
use std::time::Duration;

use synchrogit::clock;
use synchrogit::config::{load_from_path, watcher};
use synchrogit::runtime::Supervisor;

#[tokio::test]
async fn manual_reload_updates_repos_and_keeps_old_config_on_parse_error() {
    clock::init_local_offset();
    let tmp = tempfile::tempdir().unwrap();
    let repo_a = tmp.path().join("a");
    let repo_b = tmp.path().join("b");
    init_repo(&repo_a);
    init_repo(&repo_b);

    let config_path = tmp.path().join("config.toml");
    write_config(&config_path, &[("a", &repo_a)]);

    let supervisor = Supervisor::spawn_loaded(load_from_path(&config_path).unwrap()).unwrap();
    let control = supervisor.control();
    assert_eq!(repo_names(control.status()), vec!["a"]);

    write_config(&config_path, &[("a", &repo_a), ("b", &repo_b)]);
    let report = control.reload().await.unwrap();
    assert_eq!(report.added, vec!["b"]);
    assert_eq!(report.removed, Vec::<String>::new());
    assert_eq!(repo_names(control.status()), vec!["a", "b"]);

    write_config(&config_path, &[("b", &repo_b)]);
    let report = control.reload().await.unwrap();
    assert_eq!(report.added, Vec::<String>::new());
    assert_eq!(report.removed, vec!["a"]);
    assert_eq!(repo_names(control.status()), vec!["b"]);

    std::fs::write(&config_path, "not = [valid").unwrap();
    let err = control.reload().await.unwrap_err();
    assert!(err.contains("failed to parse TOML"), "{err}");
    assert_eq!(repo_names(control.status()), vec!["b"]);

    supervisor.shutdown().await;
}

#[tokio::test]
async fn config_watcher_notices_atomic_replace() {
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    std::fs::write(&config_path, "version = 1\n").unwrap();

    let mut watcher = watcher::spawn(&config_path).unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;
    let next = tmp.path().join("config.toml.tmp");
    std::fs::write(&next, "version = 2\n").unwrap();
    std::fs::rename(&next, &config_path).unwrap();

    tokio::time::timeout(Duration::from_secs(3), watcher.rx.recv())
        .await
        .expect("config watcher event timed out")
        .expect("config watcher channel closed");
}

fn write_config(path: &Path, repos: &[(&str, &Path)]) {
    let mut text = String::from(
        r#"[defaults]
interval = "1h"
debounce = "10ms"
auto-pull = false
auto-push = false
"#,
    );

    for (name, repo_path) in repos {
        text.push_str(&format!(
            r#"
[[repo]]
name = "{name}"
path = "{}"
"#,
            toml_path(repo_path),
        ));
    }

    std::fs::write(path, text).unwrap();
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

fn repo_names(status: Vec<synchrogit::ipc::protocol::RepoStatus>) -> Vec<String> {
    status.into_iter().map(|repo| repo.name).collect()
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}
