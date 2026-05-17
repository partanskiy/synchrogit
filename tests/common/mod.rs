use std::path::Path;
use std::process::Command;

pub fn run_git(dir: &Path, args: &[&str]) {
    let _ = git_output(dir, args);
}

pub fn git_stdout(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(git_output(dir, args).stdout).expect("git stdout should be utf-8")
}

fn git_output(dir: &Path, args: &[&str]) -> std::process::Output {
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
    output
}

pub fn init_bare(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    run_git(dir, &["init", "-q", "--bare", "-b", "main"]);
}

pub fn clone(remote: &Path, target: &Path) {
    let parent = target.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    let output = Command::new("git")
        .args([
            "clone",
            "-q",
            "-b",
            "main",
            remote.to_str().unwrap(),
            target.to_str().unwrap(),
        ])
        .output()
        .expect("clone spawn");

    // First push to a freshly-bare repo defines its initial branch. If the
    // bare repo has no `main` yet, `git clone -b main` errors out — fall back
    // to a branch-less clone and create the branch on the working copy.
    if !output.status.success() {
        let _ = Command::new("git")
            .args([
                "clone",
                "-q",
                remote.to_str().unwrap(),
                target.to_str().unwrap(),
            ])
            .output();
        run_git(target, &["checkout", "-b", "main"]);
    }
    run_git(target, &["config", "user.email", "test@example.com"]);
    run_git(target, &["config", "user.name", "Test User"]);
}

pub fn commit_file(dir: &Path, filename: &str, content: &str, msg: &str) {
    std::fs::write(dir.join(filename), content).unwrap();
    run_git(dir, &["add", "-A"]);
    run_git(dir, &["commit", "-q", "-m", msg]);
}
