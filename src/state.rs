use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyncOutcome {
    #[default]
    NoOp,
    Committed,
    Pulled,
    Pushed,
    Conflict,
    Error,
}

impl SyncOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoOp => "no-op",
            Self::Committed => "committed",
            Self::Pulled => "pulled",
            Self::Pushed => "pushed",
            Self::Conflict => "conflict",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for SyncOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LastSync {
    pub committed_at: Option<String>,
    pub pulled_at: Option<String>,
    pub pushed_at: Option<String>,
    pub last_cycle_at: Option<String>,
    pub last_outcome: SyncOutcome,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoState {
    pub name: String,
    pub path: PathBuf,
    pub current_branch: Option<String>,
    pub upstream: Option<String>,
    pub running: bool,
    pub last_sync: LastSync,
}

impl RepoState {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            current_branch: None,
            upstream: None,
            running: false,
            last_sync: LastSync::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CycleReport {
    pub committed: bool,
    pub pulled: bool,
    pub pushed: bool,
    pub conflict: bool,
    pub fetch_failed: Option<String>,
    pub push_skipped: Option<String>,
    pub error: Option<String>,
}

impl CycleReport {
    pub fn summary(&self) -> SyncOutcome {
        if self.error.is_some() {
            return SyncOutcome::Error;
        }
        if self.conflict {
            return SyncOutcome::Conflict;
        }
        if self.pushed {
            return SyncOutcome::Pushed;
        }
        if self.pulled {
            return SyncOutcome::Pulled;
        }
        if self.committed {
            return SyncOutcome::Committed;
        }
        SyncOutcome::NoOp
    }

    pub fn failure_message(&self) -> Option<String> {
        self.error
            .clone()
            .or_else(|| {
                self.fetch_failed
                    .clone()
                    .map(|e| format!("fetch failed: {e}"))
            })
            .or_else(|| {
                self.push_skipped
                    .clone()
                    .map(|e| format!("push skipped: {e}"))
            })
    }
}
