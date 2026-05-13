#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SyncOutcome {
    #[default]
    NoOp,
    Committed,
    Pulled,
    Pushed,
    Conflict,
    Error,
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
}
