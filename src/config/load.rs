use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use super::model::{Config, ConflictPolicy, DefaultsConfig, RepoConfig};
use crate::error::{Result, SynchrogitError};

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: Config,
}

pub fn load() -> Result<LoadedConfig> {
    load_from_candidates(config_candidates())
}

pub fn load_from_path(path: impl AsRef<Path>) -> Result<LoadedConfig> {
    let path = path.as_ref().to_path_buf();
    let text = fs::read_to_string(&path).map_err(|source| SynchrogitError::ConfigRead {
        path: path.clone(),
        source,
    })?;
    let config = parse_str(&text).map_err(|e| match e {
        SynchrogitError::Config(msg) => {
            SynchrogitError::Config(format!("{}: {msg}", path.display()))
        }
        other => other,
    })?;
    Ok(LoadedConfig { path, config })
}

pub fn load_from_candidates<I>(candidates: I) -> Result<LoadedConfig>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut checked = Vec::new();
    for path in candidates {
        checked.push(path.clone());
        if path.is_file() {
            return load_from_path(path);
        }
    }

    Err(SynchrogitError::ConfigNotFound {
        checked: checked
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
    })
}

pub fn parse_str(source: &str) -> Result<Config> {
    let raw: RawConfig = toml::from_str(source)
        .map_err(|e| SynchrogitError::Config(format!("failed to parse TOML: {e}")))?;
    raw.try_into()
}

pub fn config_candidates() -> Vec<PathBuf> {
    let home = env::var_os("HOME").map(PathBuf::from);
    let xdg = env::var_os("XDG_CONFIG_HOME").and_then(|v| {
        if v.is_empty() {
            None
        } else {
            Some(PathBuf::from(v))
        }
    });
    config_candidates_from(home.as_deref(), xdg.as_deref())
}

fn config_candidates_from(home: Option<&Path>, xdg_config_home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(xdg) = xdg_config_home {
        candidates.push(xdg.join("synchrogit/config.toml"));
    }
    if let Some(home) = home {
        candidates.push(home.join(".config/synchrogit/config.toml"));
    }
    candidates.push(PathBuf::from("/etc/synchrogit/config.toml"));

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    defaults: RawDefaultsConfig,

    #[serde(default, rename = "repo")]
    repos: Vec<RawRepoConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawDefaultsConfig {
    interval: Option<String>,
    debounce: Option<String>,
    commit_template: Option<String>,
    conflict_policy: Option<ConflictPolicy>,
    auto_push: Option<bool>,
    auto_pull: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawRepoConfig {
    name: Option<String>,
    path: String,
    branch: Option<String>,
    remote: Option<String>,
    interval: Option<String>,
    debounce: Option<String>,
    commit_template: Option<String>,
    auto_push: Option<bool>,
    auto_pull: Option<bool>,
    #[serde(default)]
    ignore: Vec<String>,
}

impl TryFrom<RawConfig> for Config {
    type Error = SynchrogitError;

    fn try_from(raw: RawConfig) -> Result<Self> {
        let defaults = DefaultsConfig {
            interval: parse_duration_opt(raw.defaults.interval.as_deref(), "defaults.interval")?
                .unwrap_or(DefaultsConfig::default().interval),
            debounce: parse_duration_opt(raw.defaults.debounce.as_deref(), "defaults.debounce")?
                .unwrap_or(DefaultsConfig::default().debounce),
            commit_template: raw
                .defaults
                .commit_template
                .unwrap_or_else(|| DefaultsConfig::default().commit_template),
            conflict_policy: raw.defaults.conflict_policy.unwrap_or_default(),
            auto_push: raw.defaults.auto_push.unwrap_or(true),
            auto_pull: raw.defaults.auto_pull.unwrap_or(true),
        };

        let repos = raw
            .repos
            .into_iter()
            .map(RepoConfig::try_from)
            .collect::<Result<Vec<_>>>()?;

        Config::new(defaults, repos)
    }
}

impl TryFrom<RawRepoConfig> for RepoConfig {
    type Error = SynchrogitError;

    fn try_from(raw: RawRepoConfig) -> Result<Self> {
        let path = expand_path(&raw.path)?;
        let name = raw.name.unwrap_or_else(|| infer_name(&path));

        Ok(Self {
            name,
            path,
            branch: raw.branch,
            remote: raw.remote,
            interval: parse_duration_opt(raw.interval.as_deref(), "repo.interval")?,
            debounce: parse_duration_opt(raw.debounce.as_deref(), "repo.debounce")?,
            commit_template: raw.commit_template,
            auto_push: raw.auto_push,
            auto_pull: raw.auto_pull,
            ignore: raw.ignore,
        })
    }
}

fn parse_duration_opt(raw: Option<&str>, field: &str) -> Result<Option<Duration>> {
    raw.map(|s| {
        humantime::parse_duration(s).map_err(|e| {
            SynchrogitError::Config(format!("invalid duration for {field} `{s}`: {e}"))
        })
    })
    .transpose()
}

fn expand_path(raw: &str) -> Result<PathBuf> {
    let expanded = shellexpand::full(raw)
        .map_err(|e| SynchrogitError::Config(format!("failed to expand path `{raw}`: {e}")))?;
    Ok(PathBuf::from(expanded.into_owned()))
}

fn infer_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_follow_xdg_then_home_then_etc() {
        let home = Path::new("/home/ilya");
        let xdg = Path::new("/tmp/config");
        assert_eq!(
            config_candidates_from(Some(home), Some(xdg)),
            vec![
                PathBuf::from("/tmp/config/synchrogit/config.toml"),
                PathBuf::from("/home/ilya/.config/synchrogit/config.toml"),
                PathBuf::from("/etc/synchrogit/config.toml"),
            ]
        );
    }
}
