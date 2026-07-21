use std::process::Stdio;

use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;
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
        let copy_rel = conflict_copy_path(f, host, &suffix);

        // Extract the local-HEAD version as bytes. Some odd states (e.g.
        // added-by-them / deleted-by-us) have no `:2:` entry; tolerate by
        // skipping the copy but still resolving the conflict.
        let mut copy_written = false;
        if let Ok(local_bytes) = capture_show(git, &format!(":2:{f}")).await {
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

// The conflict marker goes before the file extension, not after it, so the
// copy keeps the original type and stays visible to extension-filtering tools
// (Obsidian only indexes known extensions, editors keep syntax highlighting).
fn conflict_copy_path(f: &str, host: &str, suffix: &str) -> String {
    match std::path::Path::new(f).extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let stem = &f[..f.len() - ext.len() - 1];
            format!("{stem}.conflict-{host}-{suffix}.{ext}")
        }
        None => format!("{f}.conflict-{host}-{suffix}"),
    }
}

async fn capture_show(git: &Git, spec: &str) -> Result<Vec<u8>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&git.repo)
        .args(["show", spec])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.kill_on_drop(true);
    let out = timeout(git.timeout(), cmd.output())
        .await
        .map_err(|_| SynchrogitError::GitTimeout {
            args: vec!["show".to_string(), spec.to_string()],
            timeout: git.timeout(),
        })?
        .map_err(SynchrogitError::GitSpawn)?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(SynchrogitError::Other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::conflict_copy_path;

    #[test]
    fn extension_moves_after_conflict_marker() {
        assert_eq!(
            conflict_copy_path("note.md", "acchan", "20260721-144423"),
            "note.conflict-acchan-20260721-144423.md"
        );
        assert_eq!(
            conflict_copy_path("02_KB/Git/Git - Head.md", "acchan", "s"),
            "02_KB/Git/Git - Head.conflict-acchan-s.md"
        );
    }

    #[test]
    fn no_extension_appends_marker() {
        assert_eq!(
            conflict_copy_path("Makefile", "h", "s"),
            "Makefile.conflict-h-s"
        );
        assert_eq!(
            conflict_copy_path(".gitignore", "h", "s"),
            ".gitignore.conflict-h-s"
        );
    }

    #[test]
    fn dot_in_directory_is_not_an_extension() {
        assert_eq!(
            conflict_copy_path("dir.d/README", "h", "s"),
            "dir.d/README.conflict-h-s"
        );
    }
}
