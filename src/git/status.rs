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

    pub async fn head_rev(&self) -> Result<String> {
        Ok(self.run(["rev-parse", "@"]).await?.stdout_trim())
    }

    pub async fn upstream_rev(&self) -> Result<String> {
        Ok(self.run(["rev-parse", "@{u}"]).await?.stdout_trim())
    }
}
