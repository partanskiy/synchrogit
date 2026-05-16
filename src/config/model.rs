use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::clock::DEFAULT_COMMIT_TEMPLATE;
use crate::error::{Result, SynchrogitError};
use crate::worker::WorkerConfig;

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(15);
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub defaults: DefaultsConfig,
    pub repos: Vec<RepoConfig>,
}

impl Config {
    pub fn new(defaults: DefaultsConfig, repos: Vec<RepoConfig>) -> Result<Self> {
        let cfg = Self { defaults, repos };
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn resolved_repos(&self) -> Vec<ResolvedRepoConfig> {
        self.repos
            .iter()
            .map(|repo| repo.resolved(&self.defaults))
            .collect()
    }

    fn validate(&self) -> Result<()> {
        if self.repos.is_empty() {
            return Err(SynchrogitError::Config(
                "at least one [[repo]] entry is required".into(),
            ));
        }

        let mut names = HashSet::new();
        for repo in &self.repos {
            if repo.name.trim().is_empty() {
                return Err(SynchrogitError::Config(
                    "repo names must not be empty".into(),
                ));
            }
            if !names.insert(repo.name.clone()) {
                return Err(SynchrogitError::Config(format!(
                    "duplicate repo name `{}`",
                    repo.name
                )));
            }
            if !repo.path.is_absolute() {
                return Err(SynchrogitError::Config(format!(
                    "repo `{}` path must be absolute after expansion: {}",
                    repo.name,
                    repo.path.display()
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultsConfig {
    pub interval: Duration,
    pub debounce: Duration,
    pub commit_template: String,
    pub conflict_policy: ConflictPolicy,
    pub auto_push: bool,
    pub auto_pull: bool,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            interval: DEFAULT_INTERVAL,
            debounce: DEFAULT_DEBOUNCE,
            commit_template: DEFAULT_COMMIT_TEMPLATE.to_string(),
            conflict_policy: ConflictPolicy::KeepRemote,
            auto_push: true,
            auto_pull: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoConfig {
    pub name: String,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub interval: Option<Duration>,
    pub debounce: Option<Duration>,
    pub commit_template: Option<String>,
    pub auto_push: Option<bool>,
    pub auto_pull: Option<bool>,
    pub ignore: Vec<String>,
}

impl RepoConfig {
    pub fn resolved(&self, defaults: &DefaultsConfig) -> ResolvedRepoConfig {
        ResolvedRepoConfig {
            name: self.name.clone(),
            path: self.path.clone(),
            branch: self.branch.clone(),
            remote: self.remote.clone(),
            interval: self.interval.unwrap_or(defaults.interval),
            debounce: self.debounce.unwrap_or(defaults.debounce),
            commit_template: self
                .commit_template
                .clone()
                .unwrap_or_else(|| defaults.commit_template.clone()),
            conflict_policy: defaults.conflict_policy,
            auto_push: self.auto_push.unwrap_or(defaults.auto_push),
            auto_pull: self.auto_pull.unwrap_or(defaults.auto_pull),
            ignore: self.ignore.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepoConfig {
    pub name: String,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub interval: Duration,
    pub debounce: Duration,
    pub commit_template: String,
    pub conflict_policy: ConflictPolicy,
    pub auto_push: bool,
    pub auto_pull: bool,
    pub ignore: Vec<String>,
}

impl From<&ResolvedRepoConfig> for WorkerConfig {
    fn from(repo: &ResolvedRepoConfig) -> Self {
        Self {
            name: repo.name.clone(),
            path: repo.path.clone(),
            interval: repo.interval,
            debounce: repo.debounce,
            commit_template: repo.commit_template.clone(),
            auto_push: repo.auto_push,
            auto_pull: repo.auto_pull,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictPolicy {
    #[default]
    KeepRemote,
}
