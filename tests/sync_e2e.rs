mod common;

use synchrogit::clock;
use synchrogit::git::{CycleParams, Git, sync_cycle};

fn setup_pair() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    clock::init_local_offset();
    let tmp = tempfile::tempdir().unwrap();
    let remote = tmp.path().join("remote.git");
    let a = tmp.path().join("a");
    let b = tmp.path().join("b");
    common::init_bare(&remote);
    // Pre-populate the bare with a single commit so `clone -b main` works on
    // both clones below.
    let bootstrap = tmp.path().join("bootstrap");
    std::fs::create_dir_all(&bootstrap).unwrap();
    common::run_git(&bootstrap, &["init", "-q", "-b", "main"]);
    common::run_git(&bootstrap, &["config", "user.email", "test@example.com"]);
    common::run_git(&bootstrap, &["config", "user.name", "Test User"]);
    common::commit_file(&bootstrap, "README.md", "init\n", "chore: init");
    common::run_git(
        &bootstrap,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    common::run_git(&bootstrap, &["push", "-q", "-u", "origin", "main"]);

    common::clone(&remote, &a);
    common::clone(&remote, &b);
    (tmp, remote, a, b)
}

#[tokio::test]
async fn commit_and_push_on_local_change() {
    let (_tmp, _remote, a, b) = setup_pair();

    std::fs::write(a.join("note.md"), "first note\n").unwrap();
    let git_a = Git::new(&a);
    let report = sync_cycle(&git_a, &CycleParams::default()).await;
    assert!(report.committed, "should have committed: {report:?}");
    assert!(report.pushed, "should have pushed: {report:?}");
    assert!(report.error.is_none(), "no error: {report:?}");

    // B picks up the new file on its next cycle.
    let git_b = Git::new(&b);
    let report_b = sync_cycle(&git_b, &CycleParams::default()).await;
    assert!(
        report_b.pulled,
        "B should have pulled the new commit: {report_b:?}"
    );
    assert!(b.join("note.md").exists(), "note.md should land in B");
}

#[tokio::test]
async fn conflict_keeps_remote_and_saves_local_copy() {
    let (_tmp, _remote, a, b) = setup_pair();

    // Seed a shared file in B and push it.
    common::commit_file(&b, "shared.md", "base\n", "feat: base");
    common::run_git(&b, &["push", "-q"]);

    // A picks up the base so we share history.
    common::run_git(&a, &["pull", "-q"]);

    // Diverge: B pushes its own version of `shared.md`.
    std::fs::write(b.join("shared.md"), "from B\n").unwrap();
    common::run_git(&b, &["add", "-A"]);
    common::run_git(&b, &["commit", "-q", "-m", "feat: from b"]);
    common::run_git(&b, &["push", "-q"]);

    // A independently commits a different change to the same file
    // (un-pushed; this is what causes the conflict).
    std::fs::write(a.join("shared.md"), "from A\n").unwrap();
    common::run_git(&a, &["add", "-A"]);
    common::run_git(&a, &["commit", "-q", "-m", "feat: from a"]);

    let git_a = Git::new(&a);
    let report = sync_cycle(&git_a, &CycleParams::default()).await;
    assert!(report.pulled, "A should have pulled: {report:?}");
    assert!(report.conflict, "expected conflict path: {report:?}");
    assert!(report.error.is_none(), "no fatal error: {report:?}");

    // Working tree now holds the remote version.
    let final_content = std::fs::read_to_string(a.join("shared.md")).unwrap();
    assert_eq!(final_content, "from B\n");

    // A `.conflict-...` sibling holds A's pre-merge version, byte-identical.
    // The marker sits before the extension so the copy stays a visible `.md`.
    let conflicts: Vec<_> = std::fs::read_dir(&a)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("shared.conflict-") && name.ends_with(".md")
        })
        .collect();
    assert_eq!(
        conflicts.len(),
        1,
        "expected exactly one .conflict copy, got: {:?}",
        conflicts.iter().map(|e| e.file_name()).collect::<Vec<_>>()
    );
    let body = std::fs::read_to_string(conflicts[0].path()).unwrap();
    assert_eq!(body, "from A\n");
}

#[tokio::test]
async fn cycle_is_noop_when_no_changes() {
    let (_tmp, _remote, a, _b) = setup_pair();
    let git = Git::new(&a);
    let report = sync_cycle(&git, &CycleParams::default()).await;
    assert!(!report.committed);
    assert!(!report.pulled);
    assert!(!report.pushed, "in-sync cycle must not push: {report:?}");
    assert!(report.error.is_none(), "no error: {report:?}");
}

#[tokio::test]
async fn push_skipped_when_remote_up_to_date() {
    let (_tmp, _remote, a, _b) = setup_pair();
    let git = Git::new(&a);

    std::fs::write(a.join("note.md"), "first note\n").unwrap();
    let first = sync_cycle(&git, &CycleParams::default()).await;
    assert!(first.pushed, "first cycle should push: {first:?}");

    let second = sync_cycle(&git, &CycleParams::default()).await;
    assert!(
        !second.pushed,
        "second cycle has nothing to push: {second:?}"
    );
    assert!(second.error.is_none(), "no error: {second:?}");

    // Same skip through the explicit-remote path.
    let third = sync_cycle(
        &git,
        &CycleParams {
            branch: Some("main"),
            remote: Some("origin"),
            ..CycleParams::default()
        },
    )
    .await;
    assert!(
        !third.pushed,
        "explicit-remote cycle has nothing to push: {third:?}"
    );
    assert!(third.error.is_none(), "no error: {third:?}");
}

#[tokio::test]
async fn branch_config_blocks_wrong_branch() {
    let (_tmp, _remote, a, _b) = setup_pair();
    std::fs::write(a.join("note.md"), "draft\n").unwrap();

    let git = Git::new(&a);
    let report = sync_cycle(
        &git,
        &CycleParams {
            branch: Some("notes"),
            auto_pull: false,
            auto_push: false,
            ..CycleParams::default()
        },
    )
    .await;

    let error = report.error.expect("branch mismatch should fail the cycle");
    assert!(error.contains("expected `notes`"), "{error}");
    assert_eq!(
        common::git_stdout(&a, &["status", "--porcelain"]),
        "?? note.md\n"
    );
}

#[tokio::test]
async fn ignore_patterns_exclude_status_and_commit() {
    let (_tmp, _remote, a, _b) = setup_pair();
    std::fs::create_dir_all(a.join("cache")).unwrap();
    std::fs::write(a.join("cache/noise.txt"), "noise\n").unwrap();

    let ignore = vec!["cache/**".to_string()];
    let git = Git::new(&a);
    let report = sync_cycle(
        &git,
        &CycleParams {
            auto_pull: false,
            auto_push: false,
            ignore: &ignore,
            ..CycleParams::default()
        },
    )
    .await;

    assert!(
        !report.committed,
        "ignored files should not commit: {report:?}"
    );
    assert!(report.error.is_none(), "no error: {report:?}");
    assert_eq!(
        common::git_stdout(&a, &["status", "--porcelain"]),
        "?? cache/\n"
    );
}

#[tokio::test]
async fn explicit_remote_pushes_without_upstream() {
    let (_tmp, _remote, a, b) = setup_pair();
    common::run_git(&a, &["branch", "--unset-upstream"]);
    std::fs::write(a.join("remote-target.md"), "via explicit remote\n").unwrap();

    let git_a = Git::new(&a);
    let report = sync_cycle(
        &git_a,
        &CycleParams {
            branch: Some("main"),
            remote: Some("origin"),
            ..CycleParams::default()
        },
    )
    .await;
    assert!(report.committed, "should have committed: {report:?}");
    assert!(report.pushed, "should have pushed via remote: {report:?}");
    assert!(report.error.is_none(), "no error: {report:?}");

    let git_b = Git::new(&b);
    let report_b = sync_cycle(&git_b, &CycleParams::default()).await;
    assert!(report_b.pulled, "B should have pulled: {report_b:?}");
    assert!(b.join("remote-target.md").exists());
}
