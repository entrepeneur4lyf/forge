use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;
use forge_app::FileWriteService;
use forge_snaps::{FileSnapshotService, ForgeSnapshotService};

pub struct ForgeFileWriteService {
    snap_service: Arc<ForgeSnapshotService>,
}

impl ForgeFileWriteService {
    pub fn new(snap_service: Arc<ForgeSnapshotService>) -> Self {
        Self { snap_service }
    }
}

#[async_trait::async_trait]
impl FileWriteService for ForgeFileWriteService {
    async fn write(&self, path: &Path, contents: Bytes) -> Result<()> {
        let _ = self.snap_service.create_snapshot(path).await?;
        Ok(forge_fs::ForgeFS::write(path, contents.to_vec()).await?)
    }
}
