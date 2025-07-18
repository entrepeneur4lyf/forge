use std::path::Path;
use std::sync::Arc;

use forge_services::{FileRemoverInfra, SnapshotInfra};

#[derive(Default)]
pub struct ForgeFileRemoveService<S> {
    snaps: Arc<S>,
}

impl<S> ForgeFileRemoveService<S> {
    pub fn new(snaps: Arc<S>) -> Self {
        Self { snaps }
    }
}

#[async_trait::async_trait]
impl<S: SnapshotInfra> FileRemoverInfra for ForgeFileRemoveService<S> {
    async fn remove(&self, path: &Path) -> anyhow::Result<()> {
        let _ = self.snaps.create_snapshot(path).await?;
        Ok(forge_fs::ForgeFS::remove_file(path).await?)
    }
}
