//! In-process Git implementation. Never retries an external Git failure with
//! another backend; repository policy is shared with the command-line backend.
use super::{cmd::GitOutput, operation::Operation};
use crate::error::{Result, SynchrogitError};
use git2::{
    Cred, CredentialType, Error, ErrorCode, IndexAddOption, Pathspec, PathspecFlags,
    RemoteCallbacks, Repository,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

type GitResult<T> = std::result::Result<T, Error>;

// Mobile credentials live in memory only. Android persists them encrypted with
// a Keystore key, outside config.toml and outside the repository.
#[derive(Clone)]
pub struct Credentials {
    pub url: String,
    pub username: String,
    pub password: String,
}
static CREDENTIALS: OnceLock<RwLock<BTreeMap<PathBuf, Credentials>>> = OnceLock::new();
pub fn set_credentials(path: PathBuf, credentials: Option<Credentials>) {
    let mut map = CREDENTIALS
        .get_or_init(Default::default)
        .write()
        .expect("credentials lock");
    if let Some(credentials) = credentials {
        map.insert(path, credentials);
    } else {
        map.remove(&path);
    }
}

pub(crate) fn execute(path: &Path, operation: Operation, timeout: Duration) -> Result<GitOutput> {
    let started = Instant::now();
    let args = operation.args();
    let result = perform(path, operation, started + timeout);
    match result {
        Ok(stdout) => Ok(GitOutput {
            stdout,
            stderr: String::new(),
        }),
        Err(_) if started.elapsed() >= timeout => {
            Err(SynchrogitError::GitTimeout { args, timeout })
        }
        Err(error) => Err(SynchrogitError::GitFailed {
            args,
            code: 1,
            stderr: error.message().into(),
        }),
    }
}

fn perform(path: &Path, operation: Operation, deadline: Instant) -> GitResult<Vec<u8>> {
    let repo = Repository::open(path)?;
    let text = |s: String| Ok(s.into_bytes());
    match operation {
        Operation::WorkTree => {
            if repo.is_bare() {
                return Err(Error::from_str("path is not a Git work tree"));
            }
            text("true".into())
        }
        Operation::Branch => text(branch(&repo)?),
        Operation::Upstream => {
            let local = repo.find_branch(&branch(&repo)?, git2::BranchType::Local)?;
            text(
                local
                    .upstream()?
                    .name()?
                    .ok_or_else(|| Error::from_str("non-UTF8 upstream"))?
                    .into(),
            )
        }
        Operation::Rev(spec) => {
            if let Some((stage, name)) = stage_spec(&spec) {
                let index = repo.index()?;
                let entry = index
                    .get_path(Path::new(name), stage)
                    .ok_or_else(|| Error::from_str("index stage does not exist"))?;
                text(entry.id.to_string())
            } else {
                text(repo.revparse_single(&spec)?.id().to_string())
            }
        }
        Operation::GitDir => text(repo.path().to_string_lossy().into_owned()),
        Operation::Status(ignore) => {
            let excluded = exclusions(&ignore)?;
            let mut options = git2::StatusOptions::new();
            options.include_untracked(true).recurse_untracked_dirs(true);
            let statuses = repo.statuses(Some(&mut options))?;
            let mut out = Vec::new();
            for entry in statuses.iter() {
                let name = entry
                    .path()
                    .map_err(|_| Error::from_str("non-UTF8 status path"))?;
                if !is_excluded(&excluded, Path::new(name))
                    && entry.status() != git2::Status::CURRENT
                {
                    out.extend_from_slice(name.as_bytes());
                    out.push(0);
                }
            }
            Ok(out)
        }
        Operation::StageAll(ignore) => {
            let excluded = exclusions(&ignore)?;
            let mut index = repo.index()?;
            let mut filter =
                |path: &Path, _: &[u8]| if is_excluded(&excluded, path) { 1 } else { 0 };
            index.add_all(["*"], IndexAddOption::DEFAULT, Some(&mut filter))?;
            index.update_all(["*"], Some(&mut filter))?;
            index.write()?;
            Ok(Vec::new())
        }
        Operation::Commit(message) => {
            commit(&repo, &message)?;
            Ok(Vec::new())
        }
        Operation::Fetch(remote) => {
            let name = match remote {
                Some(name) => name,
                None => upstream_remote(&repo)?,
            };
            if name == "." {
                return Ok(Vec::new());
            }
            let mut remote = repo.find_remote(&name)?;
            let mut options = git2::FetchOptions::new();
            options.remote_callbacks(callbacks(&repo, deadline)?);
            remote.fetch(&[] as &[&str], Some(&mut options), None)?;
            Ok(Vec::new())
        }
        Operation::Merge(rev) => {
            merge(&repo, &rev)?;
            Ok(Vec::new())
        }
        Operation::AbortMerge => {
            if repo.state() == git2::RepositoryState::Merge {
                return Err(Error::from_str(
                    "an unfinished merge needs to be resolved before synchronization",
                ));
            }
            Ok(Vec::new())
        }
        Operation::Push {
            remote,
            branch: target,
        } => {
            let (name, target) = match remote {
                Some(name) => (
                    name,
                    format!(
                        "refs/heads/{}",
                        target.ok_or_else(|| Error::from_str("missing push branch"))?
                    ),
                ),
                None => {
                    let branch = branch(&repo)?;
                    let config = repo.config()?;
                    (
                        upstream_remote(&repo)?,
                        config.get_string(&format!("branch.{branch}.merge"))?,
                    )
                }
            };
            let mut remote = repo.find_remote(&name)?;
            let mut options = git2::PushOptions::new();
            let mut cb = callbacks(&repo, deadline)?;
            cb.push_update_reference(|_, status| match status {
                Some(error) => Err(Error::from_str(error)),
                None => Ok(()),
            });
            options.remote_callbacks(cb);
            remote.push(&[format!("HEAD:{target}")], Some(&mut options))?;
            Ok(Vec::new())
        }
        Operation::Conflicts => {
            let index = repo.index()?;
            let mut names = std::collections::BTreeSet::new();
            for conflict in index.conflicts()? {
                let c = conflict?;
                for entry in [c.ancestor, c.our, c.their].into_iter().flatten() {
                    names.insert(entry.path);
                }
            }
            Ok(names
                .into_iter()
                .flat_map(|mut name| {
                    name.push(0);
                    name
                })
                .collect())
        }
        Operation::Blob(spec) => {
            let oid = if let Some((stage, path)) = stage_spec(&spec) {
                repo.index()?
                    .get_path(Path::new(path), stage)
                    .ok_or_else(|| Error::from_str("missing index stage"))?
                    .id
            } else {
                repo.revparse_single(&spec)?.id()
            };
            Ok(repo.find_blob(oid)?.content().to_vec())
        }
        Operation::KeepRemote(name) => {
            let mut index = repo.index()?;
            let path = Path::new(&name);
            if index.get_path(path, 3).is_some() {
                let mut checkout = git2::build::CheckoutBuilder::new();
                checkout.force().use_theirs(true).path(path);
                repo.checkout_index(Some(&mut index), Some(&mut checkout))?;
                index.conflict_remove(path)?;
                index.add_path(path)?;
            } else {
                let absolute = repo
                    .workdir()
                    .ok_or_else(|| Error::from_str("missing worktree"))?
                    .join(path);
                match std::fs::remove_file(absolute) {
                    Ok(()) => (),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                    Err(error) => return Err(Error::from_str(&error.to_string())),
                }
                index.conflict_remove(path)?;
                match index.remove_path(path) {
                    Ok(()) => (),
                    Err(e) if e.code() == ErrorCode::NotFound => (),
                    Err(e) => return Err(e),
                }
            }
            index.write()?;
            Ok(Vec::new())
        }
        Operation::Stage(path) => {
            let mut index = repo.index()?;
            index.add_path(Path::new(&path))?;
            index.write()?;
            Ok(Vec::new())
        }
    }
}

fn stage_spec(spec: &str) -> Option<(i32, &str)> {
    let rest = spec.strip_prefix(':')?;
    let (stage, name) = rest.split_once(':')?;
    Some((stage.parse().ok()?, name))
}
fn branch(repo: &Repository) -> GitResult<String> {
    let head = repo.find_reference("HEAD")?;
    head.symbolic_target()
        .map_err(|_| Error::from_str("non-UTF8 HEAD"))?
        .and_then(|name| name.strip_prefix("refs/heads/"))
        .map(str::to_owned)
        .ok_or_else(|| Error::from_str("HEAD is detached"))
}
fn upstream_remote(repo: &Repository) -> GitResult<String> {
    repo.config()?
        .get_string(&format!("branch.{}.remote", branch(repo)?))
}
fn exclusions(patterns: &[String]) -> GitResult<Option<Pathspec>> {
    if patterns.iter().any(|p| p.starts_with(':')) {
        return Err(Error::from_str(
            "embedded Git supports plain ignore pathspecs; Git pathspec magic requires external Git",
        ));
    }
    if patterns.is_empty() {
        Ok(None)
    } else {
        Pathspec::new(patterns).map(Some)
    }
}
fn is_excluded(spec: &Option<Pathspec>, path: &Path) -> bool {
    spec.as_ref()
        .is_some_and(|spec| spec.matches_path(path, PathspecFlags::DEFAULT))
}
fn commit(repo: &Repository, message: &str) -> GitResult<()> {
    let mut index = repo.index()?;
    let tree = repo.find_tree(index.write_tree()?)?;
    let mut parents = Vec::new();
    match repo.head().and_then(|head| head.peel_to_commit()) {
        Ok(head) => parents.push(head),
        Err(e) if matches!(e.code(), ErrorCode::UnbornBranch | ErrorCode::NotFound) => (),
        Err(e) => return Err(e),
    }
    if repo.state() == git2::RepositoryState::Merge {
        let merge_head = std::fs::read_to_string(repo.path().join("MERGE_HEAD"))
            .map_err(|e| Error::from_str(&e.to_string()))?;
        for oid in merge_head.lines() {
            parents.push(repo.find_commit(git2::Oid::from_str(oid)?)?);
        }
    } else if parents
        .first()
        .is_some_and(|head| head.tree_id() == tree.id())
    {
        return Err(Error::from_str("nothing to commit"));
    }
    let signature = repo.signature()?;
    let refs: Vec<_> = parents.iter().collect();
    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &refs)?;
    if repo.state() == git2::RepositoryState::Merge {
        repo.cleanup_state()?;
    }
    Ok(())
}
fn merge(repo: &Repository, rev: &str) -> GitResult<()> {
    let object = repo.revparse_single(rev)?;
    let annotated = repo.find_annotated_commit(object.id())?;
    let (analysis, _) = repo.merge_analysis(&[&annotated])?;
    if analysis.is_up_to_date() {
        return Ok(());
    }
    if analysis.is_fast_forward() {
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.safe();
        repo.checkout_tree(&object, Some(&mut checkout))?;
        repo.head()?
            .set_target(object.id(), "synchrogit: fast-forward")?;
    } else {
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.safe().allow_conflicts(true);
        repo.merge(&[&annotated], None, Some(&mut checkout))?;
        if repo.index()?.has_conflicts() {
            return Err(Error::from_str("merge conflicts"));
        }
        commit(repo, &format!("Merge {rev}"))?;
    }
    Ok(())
}
fn callbacks(repo: &Repository, deadline: Instant) -> GitResult<RemoteCallbacks<'static>> {
    callbacks_for(
        repo.config()?,
        repo.workdir().unwrap_or(repo.path()),
        deadline,
    )
}

fn callbacks_for(
    config: git2::Config,
    path: &Path,
    deadline: Instant,
) -> GitResult<RemoteCallbacks<'static>> {
    let credentials = CREDENTIALS
        .get_or_init(Default::default)
        .read()
        .expect("credentials lock")
        .get(path)
        .cloned();
    let mut attempts = 0;
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |url, username, allowed| {
        attempts += 1;
        if Instant::now() >= deadline || attempts > 3 {
            return Err(Error::from_str("authentication failed or timed out"));
        }
        if let Some(ref credentials) = credentials {
            if credentials.url != url {
                return Err(Error::from_str(
                    "refusing to send credentials to a different URL",
                ));
            }
            if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
                return Cred::userpass_plaintext(&credentials.username, &credentials.password);
            }
        }
        if allowed.contains(CredentialType::SSH_KEY) {
            return Cred::ssh_key_from_agent(username.unwrap_or("git"));
        }
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            return Cred::credential_helper(&config, url, username);
        }
        if allowed.contains(CredentialType::USERNAME) {
            return Cred::username(username.unwrap_or("git"));
        }
        Cred::default()
    });
    callbacks.transfer_progress(move |_| Instant::now() < deadline);
    callbacks.sideband_progress(move |_| Instant::now() < deadline);
    callbacks.push_transfer_progress(move |_, _, _| {});
    Ok(callbacks)
}

/// Clone for mobile onboarding; the caller supplies credentials before calling.
/// Rejects nonempty destinations using libgit2's normal clone checks.
pub fn clone_repository(
    url: &str,
    path: &Path,
    name: &str,
    email: &str,
    timeout: Duration,
) -> Result<()> {
    let result = (|| -> GitResult<()> {
        let mut fetch = git2::FetchOptions::new();
        fetch.remote_callbacks(callbacks_for(
            git2::Config::open_default()?,
            path,
            Instant::now() + timeout,
        )?);
        let repo = git2::build::RepoBuilder::new()
            .fetch_options(fetch)
            .clone(url, path)?;
        let mut config = repo.config()?;
        config.set_str("user.name", name)?;
        config.set_str("user.email", email)?;
        Ok(())
    })();
    result.map_err(|e| SynchrogitError::Other(e.to_string()))
}
