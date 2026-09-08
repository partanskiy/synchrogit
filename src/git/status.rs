use super::{cmd::Git, operation::Operation};
use crate::error::{Result, SynchrogitError};
use std::path::PathBuf;

impl Git {
    async fn probe(&self, op: Operation) -> Result<bool> {
        match self.execute(op).await {
            Ok(_) => Ok(true),
            Err(SynchrogitError::GitFailed { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }
    pub async fn is_inside_work_tree(&self) -> Result<bool> {
        // Preserve ownership, permission and repository errors for diagnosis.
        Ok(self.execute(Operation::WorkTree).await?.stdout_trim() == "true")
    }
    pub async fn has_upstream(&self) -> Result<bool> {
        self.probe(Operation::Upstream).await
    }
    pub async fn current_branch(&self) -> Result<String> {
        Ok(self.execute(Operation::Branch).await?.stdout_trim())
    }
    pub async fn upstream_name(&self) -> Result<String> {
        Ok(self.execute(Operation::Upstream).await?.stdout_trim())
    }
    pub async fn rev_exists(&self, rev: &str) -> Result<bool> {
        self.probe(Operation::Rev(rev.into())).await
    }
    pub async fn git_dir(&self) -> Result<PathBuf> {
        Ok(self
            .repo
            .join(self.execute(Operation::GitDir).await?.stdout_trim()))
    }
    pub async fn porcelain(&self) -> Result<Vec<u8>> {
        self.porcelain_with_ignore(&[]).await
    }
    pub async fn porcelain_with_ignore(&self, ignore: &[String]) -> Result<Vec<u8>> {
        Ok(self
            .execute(Operation::Status(ignore.to_vec()))
            .await?
            .stdout)
    }
    pub async fn add_all_with_ignore(&self, ignore: &[String]) -> Result<()> {
        self.execute(Operation::StageAll(ignore.to_vec())).await?;
        Ok(())
    }
    pub async fn head_rev(&self) -> Result<String> {
        self.rev_parse("HEAD").await
    }
    pub async fn upstream_rev(&self) -> Result<String> {
        self.rev_parse("@{u}").await
    }
    pub async fn rev_parse(&self, rev: &str) -> Result<String> {
        Ok(self
            .execute(Operation::Rev(rev.into()))
            .await?
            .stdout_trim())
    }
}
