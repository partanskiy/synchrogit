use std::path::Path;
use std::process::Stdio;

use tokio::fs;
use tokio::process::Command;
use tracing::info;

use super::cmd::Git;
use crate::clock::{conflict_suffix, now_local, render_commit_message};
use crate::error::{Result, SynchrogitError};

pub async fn resolve_conflicts(git: &Git, template: &str, host: &str) -> Result<Vec<String>> {
    let out = git
        .run(["diff", "--name-only", "--diff-filter=U", "-z"])
        .await?;

    let files: Vec<&[u8]> = out
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .collect();
    if files.is_empty() {
        return Ok(Vec::new());
    }

    let suffix = conflict_suffix(now_local());
    let mut saved = Vec::with_capacity(files.len());

    for f_bytes in files {
        let f = std::str::from_utf8(f_bytes)
            .map_err(|_| SynchrogitError::Other("non-utf8 conflict path".into()))?;
        let copy_rel = format!("{f}.conflict-{host}-{suffix}");

        // Extract the local-HEAD version as bytes. Some odd states (e.g.
        // added-by-them / deleted-by-us) have no `:2:` entry; tolerate by
        // skipping the copy but still resolving the conflict.
        let mut copy_written = false;
        if let Ok(local_bytes) = capture_show(&git.repo, &format!(":2:{f}")).await {
            let copy_abs = git.repo.join(&copy_rel);
            if let Some(parent) = copy_abs.parent() {
                let _ = fs::create_dir_all(parent).await;
            }
            fs::write(&copy_abs, &local_bytes).await?;
            copy_written = true;
        }

        git.run(["checkout", "--theirs", "--", f]).await?;
        git.run(["add", "--", f]).await?;
        if copy_written {
            // best-effort: ignore if the copy isn't tracked (e.g. .gitignored)
            let _ = git.run(["add", "--", &copy_rel]).await;
            saved.push(copy_rel);
        }
    }

    let msg = format!(
        "{} [merge: kept remote, saved local copies]",
        render_commit_message(template, now_local(), host)
    );
    git.run(["commit", "--no-edit", "-m", &msg]).await?;

    info!(files = saved.len(), "conflict resolved");
    Ok(saved)
}

async fn capture_show(repo: &Path, spec: &str) -> Result<Vec<u8>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .args(["show", spec])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().await?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(SynchrogitError::Other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }
}
