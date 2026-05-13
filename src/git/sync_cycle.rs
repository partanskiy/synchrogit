use tracing::{info, warn};

use super::cmd::Git;
use super::conflict::resolve_conflicts;
use crate::clock::{DEFAULT_COMMIT_TEMPLATE, now_local, render_commit_message};
use crate::error::{Result, SynchrogitError};
use crate::state::CycleReport;
use crate::util::hostname::hostname;

#[derive(Debug, Clone)]
pub struct CycleParams<'a> {
    pub commit_template: &'a str,
    pub auto_push: bool,
    pub auto_pull: bool,
}

impl<'a> Default for CycleParams<'a> {
    fn default() -> Self {
        Self {
            commit_template: DEFAULT_COMMIT_TEMPLATE,
            auto_push: true,
            auto_pull: true,
        }
    }
}

pub async fn sync_cycle(git: &Git, params: &CycleParams<'_>) -> CycleReport {
    let mut report = CycleReport::default();

    // Step 0 — sanity.
    match git.is_inside_work_tree().await {
        Ok(true) => {}
        Ok(false) => {
            report.error = Some("path is not a git work tree".into());
            return report;
        }
        Err(e) => {
            report.error = Some(format!("rev-parse failed: {e}"));
            return report;
        }
    }

    let host = hostname();

    // Step 1 — commit local changes.
    match commit_local(git, params.commit_template, host).await {
        Ok(committed) => report.committed = committed,
        Err(e) => {
            report.error = Some(format!("commit failed: {e}"));
            return report;
        }
    }

    // Step 2 — pull.
    if params.auto_pull {
        match pull_step(git, params.commit_template, host).await {
            Ok((pulled, conflict, fetch_err)) => {
                report.pulled = pulled;
                report.conflict = conflict;
                report.fetch_failed = fetch_err;
            }
            Err(e) => {
                report.error = Some(format!("pull failed: {e}"));
                return report;
            }
        }
    }

    // Step 3 — push.
    if params.auto_push {
        match push_step(git).await {
            Ok(pushed) => report.pushed = pushed,
            Err(skipped) => report.push_skipped = Some(skipped),
        }
    }

    report
}

async fn commit_local(git: &Git, template: &str, host: &str) -> Result<bool> {
    let porc = git.porcelain().await?;
    if porc.is_empty() {
        return Ok(false);
    }
    git.run(["add", "-A"]).await?;
    let msg = render_commit_message(template, now_local(), host);
    match git.run(["commit", "-m", &msg]).await {
        Ok(_) => {
            info!(message = %msg, "committed");
            Ok(true)
        }
        Err(SynchrogitError::GitFailed { stderr, .. }) if stderr.contains("nothing to commit") => {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

async fn pull_step(git: &Git, template: &str, host: &str) -> Result<(bool, bool, Option<String>)> {
    if !git.has_upstream().await? {
        return Ok((false, false, None));
    }

    if let Err(SynchrogitError::GitFailed { stderr, .. }) = git.run(["fetch", "--quiet"]).await {
        let trimmed = stderr.trim().to_string();
        warn!(stderr = %trimmed, "fetch failed (offline?)");
        return Ok((false, false, Some(trimmed)));
    }

    let local = git.head_rev().await?;
    let remote = git.upstream_rev().await?;
    if local == remote {
        return Ok((false, false, None));
    }

    match git
        .run(["pull", "--no-rebase", "--no-edit", "--quiet"])
        .await
    {
        Ok(_) => {
            info!("pulled");
            Ok((true, false, None))
        }
        Err(SynchrogitError::GitFailed { .. }) => {
            let git_dir = git.git_dir().await?;
            if git_dir.join("MERGE_HEAD").exists() {
                resolve_conflicts(git, template, host).await?;
                Ok((true, true, None))
            } else {
                let _ = git.run(["merge", "--abort"]).await;
                Err(SynchrogitError::Other(
                    "pull failed without producing a merge state".into(),
                ))
            }
        }
        Err(e) => Err(e),
    }
}

async fn push_step(git: &Git) -> std::result::Result<bool, String> {
    match git.has_upstream().await {
        Ok(true) => {}
        Ok(false) => return Ok(false),
        Err(e) => return Err(format!("upstream probe failed: {e}")),
    }
    match git.run(["push", "--quiet"]).await {
        Ok(_) => {
            info!("pushed");
            Ok(true)
        }
        Err(SynchrogitError::GitFailed { stderr, .. }) => {
            let s = stderr.trim().to_string();
            warn!(stderr = %s, "push skipped");
            Err(s)
        }
        Err(e) => Err(e.to_string()),
    }
}
