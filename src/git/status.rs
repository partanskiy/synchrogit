use std::path::PathBuf;

use super::cmd::Git;
use crate::error::Result;

impl Git {
    pub async fn is_inside_work_tree(&self) -> Result<bool> {
        self.try_run(["rev-parse", "--is-inside-work-tree"]).await
    }

    pub async fn has_upstream(&self) -> Result<bool> {
        self.try_run(["rev-parse", "--abbrev-ref", "@{u}"]).await
    }

    pub async fn current_branch(&self) -> Result<String> {
        Ok(self
            .run(["symbolic-ref", "--short", "HEAD"])
            .await?
            .stdout_trim())
    }

    pub async fn upstream_name(&self) -> Result<String> {
        Ok(self
            .run(["rev-parse", "--abbrev-ref", "@{u}"])
            .await?
            .stdout_trim())
    }

    pub async fn rev_exists(&self, rev: &str) -> Result<bool> {
        self.try_run(["rev-parse", "--verify", rev]).await
    }

    pub async fn git_dir(&self) -> Result<PathBuf> {
        let out = self.run(["rev-parse", "--git-dir"]).await?;
        Ok(self.repo.join(out.stdout_trim()))
    }

    pub async fn porcelain(&self) -> Result<Vec<u8>> {
        Ok(self
            .run(["status", "--porcelain=v1", "--untracked-files=normal"])
            .await?
            .stdout)
    }

    pub async fn porcelain_with_ignore(&self, ignore: &[String]) -> Result<Vec<u8>> {
        Ok(self.run(pathspec_args(status_args(), ignore)).await?.stdout)
    }

    pub async fn add_all_with_ignore(&self, ignore: &[String]) -> Result<()> {
        self.run(pathspec_args(vec!["add".into(), "-A".into()], ignore))
            .await?;
        Ok(())
    }

    pub async fn head_rev(&self) -> Result<String> {
        Ok(self.run(["rev-parse", "@"]).await?.stdout_trim())
    }

    pub async fn upstream_rev(&self) -> Result<String> {
        Ok(self.run(["rev-parse", "@{u}"]).await?.stdout_trim())
    }

    pub async fn rev_parse(&self, rev: &str) -> Result<String> {
        Ok(self.run(["rev-parse", rev]).await?.stdout_trim())
    }
}

fn status_args() -> Vec<String> {
    vec![
        "status".into(),
        "--porcelain=v1".into(),
        "--untracked-files=normal".into(),
    ]
}

fn pathspec_args(mut args: Vec<String>, ignore: &[String]) -> Vec<String> {
    if ignore.is_empty() {
        return args;
    }

    args.push("--".into());
    args.push(".".into());
    args.extend(ignore.iter().map(|pattern| format!(":(exclude){pattern}")));
    args
}
