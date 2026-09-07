use tokio::fs;
use tracing::info;

use super::cmd::Git;
use super::operation::Operation;
use crate::clock::{conflict_suffix, now_local, render_commit_message};
use crate::error::{Result, SynchrogitError};

pub async fn resolve_conflicts(git: &Git, template: &str, host: &str) -> Result<Vec<String>> {
    let out = git.execute(Operation::Conflicts).await?;

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

        // A local deletion has no stage-2 entry to save. If the entry does
        // exist, preserve it successfully before replacing or removing the
        // original file; a read failure must not be treated as a deletion.
        let mut copy_written = false;
        let local_spec = format!(":2:{f}");
        if git.rev_exists(&local_spec).await? {
            let local_bytes = git
                .execute(Operation::Blob(local_spec.clone()))
                .await?
                .stdout;
            let copy_abs = git.repo.join(&copy_rel);
            if let Some(parent) = copy_abs.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&copy_abs, &local_bytes).await?;
            copy_written = true;
        }

        git.execute(Operation::KeepRemote(f.into())).await?;
        if copy_written {
            // best-effort: ignore if the copy isn't tracked (e.g. .gitignored)
            let _ = git.execute(Operation::Stage(copy_rel.clone())).await;
            saved.push(copy_rel);
        }
    }

    let msg = format!(
        "{} [merge: kept remote, saved local copies]",
        render_commit_message(template, now_local(), host)
    );
    git.execute(Operation::Commit(msg)).await?;

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
