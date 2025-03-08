use std::path::{Path, PathBuf};

use anyhow::Result;
use forge_app::FileSnapshotService;
use forge_domain::Environment;
use forge_snaps::{SnapshotInfo, SnapshotMetadata};

pub struct ForgeFileSnapshotService {
    inner: forge_snaps::SnapshotService,
}

impl ForgeFileSnapshotService {
    pub fn new(env: Environment) -> Self {
        Self {
            inner: forge_snaps::SnapshotService::new(env.snapshot_path()),
        }
    }
}

#[async_trait::async_trait]
impl FileSnapshotService for ForgeFileSnapshotService {
    fn snapshot_dir(&self) -> PathBuf {
        todo!()
    }

    // Creation
    // FIXME: don't depend on forge_snaps::SnapshotInfo directly
    async fn create_snapshot(&self, file_path: &Path) -> Result<SnapshotInfo> {
        todo!()
    }

    // Listing
    async fn list_snapshots(&self, file_path: &Path) -> Result<Vec<SnapshotInfo>> {
        todo!()
    }

    // Timestamp-based restoration
    async fn restore_by_timestamp(&self, file_path: &Path, timestamp: &str) -> Result<()> {
        todo!()
    }

    // Index-based restoration (0 = newest, 1 = previous version, etc.)
    async fn restore_by_index(&self, file_path: &Path, index: isize) -> Result<()> {
        todo!()
    }

    // Convenient method to restore previous version
    async fn restore_previous(&self, file_path: &Path) -> Result<()> {
        todo!()
    }

    // Metadata access
    async fn get_snapshot_by_timestamp(
        &self,
        file_path: &Path,
        timestamp: &str,
    ) -> Result<SnapshotMetadata> {
        todo!()
    }
    async fn get_snapshot_by_index(
        &self,
        file_path: &Path,
        index: isize,
    ) -> Result<SnapshotMetadata> {
        todo!()
    }

    // Global purge operation
    async fn purge_older_than(&self, days: u32) -> Result<usize> {
        todo!()
    }
}
