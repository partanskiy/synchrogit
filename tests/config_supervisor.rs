use std::path::Path;
use std::process::Command;
use std::time::Duration;

use synchrogit::clock;
use synchrogit::config::{load_from_candidates, parse_str};
use synchrogit::runtime::Supervisor;

#[test]
fn config_loads_first_existing_candidate_and_resolves_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let notes = tmp.path().join("notes");
    let wiki = tmp.path().join("wiki");
    let selected = tmp.path().join("selected.toml");
    let fallback = tmp.path().join("fallback.toml");

    std::fs::write(
        &fallback,
        format!(
            r#"
[[repo]]
path = "{}"
"#,
            toml_path(&wiki)
        ),
    )
    .unwrap();

    std::fs::write(
        &selected,
        format!(
            r#"
[defaults]
interval = "20s"
debounce = "3s"
backoff-min = "5s"
backoff-max = "45s"
git-timeout = "30s"
auto-push = false
auto-pull = true

[[repo]]
name = "notes"
path = "{}"
interval = "30s"
auto-pull = false
ignore = ["target", ".direnv"]

[[repo]]
path = "{}"
"#,
            toml_path(&notes),
            toml_path(&wiki),
        ),
    )
    .unwrap();

    let loaded =
        load_from_candidates(vec![tmp.path().join("missing.toml"), selected.clone()]).unwrap();
    assert_eq!(loaded.path, selected);

    let repos = loaded.config.resolved_repos();
    assert_eq!(repos.len(), 2);

    let notes_cfg = repos.iter().find(|r| r.name == "notes").unwrap();
    assert_eq!(notes_cfg.path, notes);
    assert_eq!(notes_cfg.interval, Duration::from_secs(30));
    assert_eq!(notes_cfg.debounce, Duration::from_secs(3));
    assert_eq!(notes_cfg.backoff_min, Duration::from_secs(5));
    assert_eq!(notes_cfg.backoff_max, Duration::from_secs(45));
    assert_eq!(notes_cfg.git_timeout, Duration::from_secs(30));
    assert!(!notes_cfg.auto_push);
    assert!(!notes_cfg.auto_pull);
    assert_eq!(notes_cfg.ignore, vec!["target", ".direnv"]);

    let wiki_cfg = repos.iter().find(|r| r.name == "wiki").unwrap();
    assert_eq!(wiki_cfg.path, wiki);
    assert_eq!(wiki_cfg.interval, Duration::from_secs(20));
    assert_eq!(wiki_cfg.debounce, Duration::from_secs(3));
    assert_eq!(wiki_cfg.git_timeout, Duration::from_secs(30));
    assert!(!wiki_cfg.auto_push);
    assert!(wiki_cfg.auto_pull);
}

#[test]
fn config_rejects_invalid_backoff_bounds() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");

    let err = parse_str(&format!(
        r#"
[defaults]
backoff-min = "0s"
backoff-max = "1s"

[[repo]]
path = "{}"
"#,
        toml_path(&repo),
    ))
    .unwrap_err()
    .to_string();
    assert!(err.contains("defaults.backoff-min must be greater than zero"));

    let err = parse_str(&format!(
        r#"
[defaults]
backoff-min = "10s"
backoff-max = "1s"

[[repo]]
path = "{}"
"#,
        toml_path(&repo),
    ))
    .unwrap_err()
    .to_string();
    assert!(err.contains("defaults.backoff-max must be greater than or equal"));
}

#[test]
fn config_rejects_invalid_timeout_and_empty_sync_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");

    let err = parse_str(&format!(
        r#"
[defaults]
git-timeout = "0s"

[[repo]]
path = "{}"
"#,
        toml_path(&repo),
    ))
    .unwrap_err()
    .to_string();
    assert!(err.contains("defaults.git-timeout must be greater than zero"));

    let err = parse_str(&format!(
        r#"
[[repo]]
path = "{}"
branch = ""
"#,
        toml_path(&repo),
    ))
    .unwrap_err()
    .to_string();
    assert!(err.contains("branch must not be empty"));

    let err = parse_str(&format!(
        r#"
[[repo]]
path = "{}"
ignore = [""]
"#,
        toml_path(&repo),
    ))
    .unwrap_err()
    .to_string();
    assert!(err.contains("ignore patterns must not be empty"));
}

#[test]
fn packaging_example_config_parses() {
    parse_str(include_str!("../packaging/config.example.toml")).unwrap();
}

#[tokio::test]
async fn supervisor_spawns_workers_and_stops_them() {
    clock::init_local_offset();
    let tmp = tempfile::tempdir().unwrap();
    let repo_a = tmp.path().join("a");
    let repo_b = tmp.path().join("b");
    init_repo(&repo_a);
    init_repo(&repo_b);

    let config = parse_str(&format!(
        r#"
[defaults]
interval = "1h"
debounce = "10ms"
auto-pull = false
auto-push = false

[[repo]]
name = "a"
path = "{}"

[[repo]]
name = "b"
path = "{}"
"#,
        toml_path(&repo_a),
        toml_path(&repo_b),
    ))
    .unwrap();

    let supervisor = Supervisor::spawn(config).unwrap();
    assert_eq!(supervisor.worker_count(), 2);
    tokio::time::sleep(Duration::from_millis(50)).await;
    supervisor.shutdown().await;
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
