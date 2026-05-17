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
    pub branch: Option<&'a str>,
    pub remote: Option<&'a str>,
    pub ignore: &'a [String],
}

impl<'a> Default for CycleParams<'a> {
    fn default() -> Self {
        Self {
            commit_template: DEFAULT_COMMIT_TEMPLATE,
            auto_push: true,
            auto_pull: true,
            branch: None,
            remote: None,
            ignore: &[],
        }
    }
}

#[derive(Debug, Clone)]
struct SyncTarget {
    branch: Option<String>,
    remote: Option<String>,
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

    let target = match resolve_target(git, params).await {
        Ok(target) => target,
        Err(e) => {
            report.error = Some(e.to_string());
            return report;
        }
    };

    let host = hostname();

    // Step 1 — commit local changes.
    match commit_local(git, params.commit_template, host, params.ignore).await {
        Ok(committed) => report.committed = committed,
        Err(e) => {
            report.error = Some(format!("commit failed: {e}"));
            return report;
        }
    }

    // Step 2 — pull.
    if params.auto_pull {
        match pull_step(git, &target, params.commit_template, host).await {
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
        match push_step(git, &target).await {
            Ok(pushed) => report.pushed = pushed,
            Err(skipped) => report.push_skipped = Some(skipped),
        }
    }

    report
}

async fn resolve_target(git: &Git, params: &CycleParams<'_>) -> Result<SyncTarget> {
    let current_branch = git.current_branch().await.ok();

    let branch = match params.branch {
        Some(expected) => {
            let current = current_branch.ok_or_else(|| {
                SynchrogitError::Other(format!("repo is not on a branch; expected `{expected}`"))
            })?;
            if current != expected {
                return Err(SynchrogitError::Other(format!(
                    "repo is on branch `{current}`, expected `{expected}`"
                )));
            }
            Some(expected.to_string())
        }
        None => current_branch,
    };

    if params.remote.is_some() && branch.is_none() {
        return Err(SynchrogitError::Other(
            "remote target requires a current branch or repo.branch".into(),
        ));
    }

    Ok(SyncTarget {
        branch,
        remote: params.remote.map(str::to_string),
    })
}

async fn commit_local(git: &Git, template: &str, host: &str, ignore: &[String]) -> Result<bool> {
    let porc = git.porcelain_with_ignore(ignore).await?;
    if porc.is_empty() {
        return Ok(false);
    }
    git.add_all_with_ignore(ignore).await?;
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

async fn pull_step(
    git: &Git,
    target: &SyncTarget,
    template: &str,
    host: &str,
) -> Result<(bool, bool, Option<String>)> {
    let remote_ref = if let Some(remote) = &target.remote {
        let branch = target
            .branch
            .as_deref()
            .ok_or_else(|| SynchrogitError::Other("remote target requires a branch".into()))?;
        let ref_name = format!("{remote}/{branch}");
        if let Err(SynchrogitError::GitFailed { stderr, .. }) =
            git.run(["fetch", "--quiet", remote]).await
        {
            let trimmed = stderr.trim().to_string();
            warn!(stderr = %trimmed, "fetch failed (offline?)");
            return Ok((false, false, Some(trimmed)));
        }
        if !git.rev_exists(&ref_name).await? {
            return Ok((false, false, None));
        }
        ref_name
    } else {
        if !git.has_upstream().await? {
            return Ok((false, false, None));
        }
        if let Err(SynchrogitError::GitFailed { stderr, .. }) = git.run(["fetch", "--quiet"]).await
        {
            let trimmed = stderr.trim().to_string();
            warn!(stderr = %trimmed, "fetch failed (offline?)");
            return Ok((false, false, Some(trimmed)));
        }
        "@{u}".to_string()
    };

    let local = git.head_rev().await?;
    let remote = git.rev_parse(&remote_ref).await?;
    if local == remote {
        return Ok((false, false, None));
    }

    let merge_args = ["merge", "--no-edit", "--quiet", remote_ref.as_str()];
    match git.run(merge_args).await {
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
                    "merge failed without producing a merge state".into(),
                ))
            }
        }
        Err(e) => Err(e),
    }
}

async fn push_step(git: &Git, target: &SyncTarget) -> std::result::Result<bool, String> {
    if let Some(remote) = &target.remote {
        let branch = target
            .branch
            .as_deref()
            .ok_or_else(|| "remote target requires a branch".to_string())?;
        let refspec = format!("HEAD:{branch}");
        return run_push(git, ["push", "--quiet", remote.as_str(), refspec.as_str()]).await;
    }

    match git.has_upstream().await {
        Ok(true) => {}
        Ok(false) => return Ok(false),
        Err(e) => return Err(format!("upstream probe failed: {e}")),
    }
    run_push(git, ["push", "--quiet"]).await
}

async fn run_push<I, S>(git: &Git, args: I) -> std::result::Result<bool, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    match git.run(args).await {
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
