/// The Git operations used by the sync engine. Policy stays in sync_cycle and
/// conflict; backends only manipulate repositories.
#[derive(Debug, Clone)]
pub(crate) enum Operation {
    WorkTree,
    Branch,
    Upstream,
    Rev(String),
    GitDir,
    Status(Vec<String>),
    StageAll(Vec<String>),
    Commit(String),
    Fetch(Option<String>),
    Merge(String),
    AbortMerge,
    Push {
        remote: Option<String>,
        branch: Option<String>,
    },
    Conflicts,
    Blob(String),
    KeepRemote(String),
    Stage(String),
}

impl Operation {
    pub fn args(&self) -> Vec<String> {
        let args: Vec<&str> = match self {
            Self::WorkTree => vec!["rev-parse", "--is-inside-work-tree"],
            Self::Branch => vec!["symbolic-ref", "--short", "HEAD"],
            Self::Upstream => vec!["rev-parse", "--abbrev-ref", "@{u}"],
            Self::Rev(rev) => vec!["rev-parse", "--verify", rev],
            Self::GitDir => vec!["rev-parse", "--git-dir"],
            Self::Status(_) => vec!["status", "--porcelain=v1", "--untracked-files=normal"],
            Self::StageAll(_) => vec!["add", "-A"],
            Self::Commit(message) => vec!["commit", "-m", message],
            Self::Fetch(remote) => {
                let mut a = vec!["fetch", "--quiet"];
                if let Some(remote) = remote {
                    a.push(remote);
                }
                a
            }
            Self::Merge(rev) => vec!["merge", "--no-edit", "--quiet", rev],
            Self::AbortMerge => vec!["merge", "--abort"],
            Self::Push { remote, branch } => {
                let mut a = vec!["push".into(), "--quiet".into()];
                if let Some(remote) = remote {
                    a.push(remote.clone());
                }
                if let Some(branch) = branch {
                    a.push(format!("HEAD:{branch}"));
                }
                return a;
            }
            Self::Conflicts => vec!["diff", "--name-only", "--diff-filter=U", "-z"],
            Self::Blob(spec) => vec!["show", spec],
            // External keep-remote is a compound operation handled by Git.
            Self::KeepRemote(path) => vec!["checkout", "--theirs", "--", path],
            Self::Stage(path) => vec!["add", "--", path],
        };
        let mut args: Vec<String> = args.into_iter().map(str::to_owned).collect();
        if let Self::Status(ignore) | Self::StageAll(ignore) = self
            && !ignore.is_empty()
        {
            args.extend(["--".into(), ".".into()]);
            args.extend(ignore.iter().map(|p| format!(":(exclude){p}")));
        }
        args
    }
}
