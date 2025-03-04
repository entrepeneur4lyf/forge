use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::{FileSnapshotService, SnapshotInfo, SnapshotMetadata};

/// Implementation of `FileSnapshotService` that provides snapshot
/// functionality for files with retention policies.
pub struct FileSnapshotServiceImpl {
    /// Base directory for storing snapshots
    snapshot_base_dir: PathBuf,
    /// Maximum number of snapshots to keep per file
    max_snapshots_per_file: usize,
    /// Default retention period in days
    default_retention_days: u32,
}

impl FileSnapshotServiceImpl {
    /// Creates a new instance with a custom snapshot directory
    pub fn new(snapshot_base_dir: PathBuf) -> Self {
        Self {
            snapshot_base_dir,
            max_snapshots_per_file: 10, // Default from requirements
            default_retention_days: 30, // Default from requirements
        }
    }

    /// Calculates a SHA-256 hash of the file path for storage organization
    fn hash_path(&self, file_path: &Path) -> String {
        let path_str = file_path.to_string_lossy().to_string();
        let mut hasher = Sha256::new();
        hasher.update(path_str.as_bytes());
        format!("{:x}", hasher.finalize()[..])
    }

    /// Gets the directory for a specific file's snapshots
    async fn get_file_snapshot_dir(&self, file_path: &Path) -> Result<PathBuf> {
        let hash = self.hash_path(file_path);
        let dir = self.snapshot_base_dir.join(hash);

        // Create the directory if it doesn't exist
        if !dir.exists() {
            create_dir_all(&dir)
                .await
                .with_context(|| format!("Failed to create snapshot directory: {:?}", dir))?;
        }

        Ok(dir)
    }

    /// Creates a snapshot filename based on the timestamp
    fn create_snapshot_filename(&self, timestamp: u64) -> String {
        format!("{}.snap", timestamp)
    }

    /// Gets the timestamp from a snapshot filename
    fn get_timestamp_from_filename(&self, filename: &str) -> Option<u64> {
        if let Some(name) = filename.strip_suffix(".snap") {
            name.parse::<u64>().ok()
        } else {
            None
        }
    }

    /// Retrieves all snapshot files for a given file, sorted by timestamp (newest first)
    async fn get_sorted_snapshots(&self, file_path: &Path) -> Result<Vec<(u64, PathBuf)>> {
        let snapshot_dir = self.get_file_snapshot_dir(file_path).await?;
        let mut snapshots = Vec::new();

        let mut entries = read_dir(&snapshot_dir)
            .await
            .with_context(|| format!("Failed to read snapshot directory: {:?}", snapshot_dir))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(timestamp) = self.get_timestamp_from_filename(filename) {
                    snapshots.push((timestamp, path));
                }
            }
        }

        // Sort by timestamp (newest first)
        snapshots.sort_by(|a, b| b.0.cmp(&a.0));

        Ok(snapshots)
    }

    /// Applies retention policy to snapshots, removing excess ones
    async fn apply_retention_policy(&self, file_path: &Path) -> Result<()> {
        let snapshots = self.get_sorted_snapshots(file_path).await?;

        // Remove excess snapshots based on max_snapshots_per_file
        if snapshots.len() > self.max_snapshots_per_file {
            for (_, path) in snapshots.iter().skip(self.max_snapshots_per_file) {
                remove_file(path)
                    .await
                    .with_context(|| format!("Failed to remove excess snapshot: {:?}", path))?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl FileSnapshotService for FileSnapshotServiceImpl {
    fn snapshot_dir(&self) -> PathBuf {
        self.snapshot_base_dir.clone()
    }

    async fn create_snapshot(&self, file_path: &Path) -> Result<SnapshotInfo> {
        // Ensure the file exists
        if !file_path.exists() {
            anyhow::bail!("File does not exist: {:?}", file_path);
        }

        // Read the file content
        let content = read(file_path)
            .await
            .with_context(|| format!("Failed to read file: {:?}", file_path))?;

        let file_size = content.len() as u64;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        // Get the snapshot directory and create it if needed
        let snapshot_dir = self.get_file_snapshot_dir(file_path).await?;
        let snapshot_filename = self.create_snapshot_filename(timestamp);
        let snapshot_path = snapshot_dir.join(&snapshot_filename);

        // Write the snapshot
        write(&snapshot_path, &content)
            .await
            .with_context(|| format!("Failed to write snapshot: {:?}", snapshot_path))?;

        // Apply retention policy
        self.apply_retention_policy(file_path).await?;

        // Create and return the SnapshotInfo
        let snapshot_info = SnapshotInfo::with_timestamp(
            timestamp,
            file_path.to_path_buf(),
            snapshot_path,
            file_size,
            0, // This is the newest snapshot, so index is 0
        );

        Ok(snapshot_info)
    }

    async fn list_snapshots(&self, file_path: &Path) -> Result<Vec<SnapshotInfo>> {
        let snapshots = self.get_sorted_snapshots(file_path).await?;
        let mut result = Vec::new();

        for (index, (timestamp, path)) in snapshots.iter().enumerate() {
            let metadata = fs::metadata(&path)
                .with_context(|| format!("Failed to get metadata for snapshot: {:?}", path))?;

            let snapshot_info = SnapshotInfo::with_timestamp(
                *timestamp,
                file_path.to_path_buf(),
                path.clone(),
                metadata.len(),
                index,
            );

            result.push(snapshot_info);
        }

        Ok(result)
    }

    async fn restore_by_timestamp(&self, file_path: &Path, timestamp: u64) -> Result<()> {
        let snapshot_metadata = self.get_snapshot_by_timestamp(file_path, timestamp).await?;

        // Write the content back to the original file
        write(file_path, &snapshot_metadata.content)
            .await
            .with_context(|| format!("Failed to restore file: {:?}", file_path))?;

        Ok(())
    }

    async fn restore_by_index(&self, file_path: &Path, index: usize) -> Result<()> {
        let snapshot_metadata = self.get_snapshot_by_index(file_path, index).await?;

        // Write the content back to the original file
        write(file_path, &snapshot_metadata.content)
            .await
            .with_context(|| format!("Failed to restore file: {:?}", file_path))?;

        Ok(())
    }

    async fn get_snapshot_by_timestamp(
        &self,
        file_path: &Path,
        timestamp: u64,
    ) -> Result<SnapshotMetadata> {
        let snapshot_dir = self.get_file_snapshot_dir(file_path).await?;
        let snapshot_filename = self.create_snapshot_filename(timestamp);
        let snapshot_path = snapshot_dir.join(snapshot_filename);

        if !snapshot_path.exists() {
            anyhow::bail!("Snapshot does not exist for timestamp {}", timestamp);
        }

        let content = read(&snapshot_path)
            .await
            .with_context(|| format!("Failed to read snapshot: {:?}", snapshot_path))?;

        let metadata = fs::metadata(&snapshot_path)
            .with_context(|| format!("Failed to get metadata for snapshot: {:?}", snapshot_path))?;

        // Find the index of this snapshot
        let snapshots = self.get_sorted_snapshots(file_path).await?;
        let index = snapshots
            .iter()
            .position(|(t, _)| *t == timestamp)
            .unwrap_or(0);

        let info = SnapshotInfo::with_timestamp(
            timestamp,
            file_path.to_path_buf(),
            snapshot_path,
            metadata.len(),
            index,
        );

        Ok(SnapshotMetadata { info, content, path_hash: self.hash_path(file_path) })
    }

    async fn get_snapshot_by_index(
        &self,
        file_path: &Path,
        index: usize,
    ) -> Result<SnapshotMetadata> {
        let snapshots = self.get_sorted_snapshots(file_path).await?;

        if index >= snapshots.len() {
            anyhow::bail!(
                "Snapshot index {} is out of bounds (max: {})",
                index,
                snapshots.len().saturating_sub(1)
            );
        }

        let (timestamp, _) = snapshots[index];
        self.get_snapshot_by_timestamp(file_path, timestamp).await
    }

    async fn purge_older_than(&self, days: u32) -> Result<usize> {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .checked_sub(Duration::from_secs(days as u64 * 24 * 60 * 60))
            .unwrap_or(Duration::from_secs(0))
            .as_secs();

        let mut removed_count = 0;

        // Iterate through all directories in the snapshot base dir
        let mut entries = read_dir(&self.snapshot_base_dir).await.with_context(|| {
            format!(
                "Failed to read base snapshot directory: {:?}",
                self.snapshot_base_dir
            )
        })?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() {
                // This is a directory for a specific file's snapshots
                let mut snapshots = read_dir(&path)
                    .await
                    .with_context(|| format!("Failed to read snapshot directory: {:?}", path))?;

                // Check each snapshot file in this directory
                while let Some(snapshot_entry) = snapshots.next_entry().await? {
                    let snapshot_path = snapshot_entry.path();

                    if let Some(filename) = snapshot_path.file_name().and_then(|n| n.to_str()) {
                        if let Some(timestamp) = self.get_timestamp_from_filename(filename) {
                            // Remove if older than the cutoff
                            if timestamp < cutoff {
                                remove_file(&snapshot_path).await.with_context(|| {
                                    format!("Failed to remove old snapshot: {:?}", snapshot_path)
                                })?;
                                removed_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs::File;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_create_snapshot() -> Result<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().to_path_buf();
        let service = FileSnapshotServiceImpl::new(base_path.join("snapshots"));

        // Create a test file
        let test_file_path = base_path.join("test.txt");
        let mut file = File::create(&test_file_path).await?;
        file.write_all(b"test content").await?;
        file.flush().await?;

        // Create a snapshot
        let snapshot_info = service.create_snapshot(&test_file_path).await?;

        // Verify the snapshot was created
        assert_eq!(snapshot_info.original_path, test_file_path);
        assert!(snapshot_info.snapshot_path.exists());
        assert_eq!(snapshot_info.size, 12); // "test content" is 12 bytes

        Ok(())
    }

    #[tokio::test]
    async fn test_list_snapshots() -> Result<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().to_path_buf();
        let service = FileSnapshotServiceImpl::new(base_path.join("snapshots"));

        // Create a test file
        let test_file_path = base_path.join("test.txt");
        let mut file = File::create(&test_file_path).await?;
        file.write_all(b"test content").await?;
        file.flush().await?;

        // Create multiple snapshots
        let _snapshot1 = service.create_snapshot(&test_file_path).await?;

        // Modify file
        let mut file = File::create(&test_file_path).await?;
        file.write_all(b"modified content").await?;
        file.flush().await?;

        let _snapshot2 = service.create_snapshot(&test_file_path).await?;

        // List snapshots
        let snapshots = service.list_snapshots(&test_file_path).await?;

        // Verify we have 2 snapshots, newest first
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].index, 0);
        assert_eq!(snapshots[1].index, 1);

        // Verify sizes
        assert_eq!(snapshots[1].size, 12); // "test content" is 12 bytes
        assert_eq!(snapshots[0].size, 16); // "modified content" is 16 bytes

        Ok(())
    }

    #[tokio::test]
    async fn test_restore_by_index() -> Result<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().to_path_buf();
        let service = FileSnapshotServiceImpl::new(base_path.join("snapshots"));

        // Create a test file
        let test_file_path = base_path.join("test.txt");
        let mut file = File::create(&test_file_path).await?;
        file.write_all(b"original content").await?;
        file.flush().await?;

        // Create a snapshot
        let _snapshot1 = service.create_snapshot(&test_file_path).await?;

        // Modify file
        let mut file = File::create(&test_file_path).await?;
        file.write_all(b"modified content").await?;
        file.flush().await?;

        // Restore to the original content
        service.restore_by_index(&test_file_path, 1).await?;

        // Verify the file has been restored
        let content = tokio::fs::read_to_string(&test_file_path).await?;
        assert_eq!(content, "original content");

        Ok(())
    }

    #[tokio::test]
    async fn test_retention_policy() -> Result<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().to_path_buf();
        let mut service = FileSnapshotServiceImpl::new(base_path.join("snapshots"));
        service.max_snapshots_per_file = 3; // Set a small limit for testing

        // Create a test file
        let test_file_path = base_path.join("test.txt");
        let mut file = File::create(&test_file_path).await?;
        file.write_all(b"content 1").await?;
        file.flush().await?;

        // Create multiple snapshots with modifications
        for i in 1..=5 {
            let _snapshot = service.create_snapshot(&test_file_path).await?;

            let mut file = File::create(&test_file_path).await?;
            file.write_all(format!("content {}", i + 1).as_bytes())
                .await?;
            file.flush().await?;
        }

        // List snapshots - should only have 3 due to retention policy
        let snapshots = service.list_snapshots(&test_file_path).await?;

        // Verify retention policy was applied
        assert_eq!(snapshots.len(), 3);

        Ok(())
    }
}
