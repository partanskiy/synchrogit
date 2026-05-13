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
    let conflicts: Vec<_> = std::fs::read_dir(&a)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("shared.md.conflict-")
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
    assert!(report.error.is_none(), "no error: {report:?}");
}
