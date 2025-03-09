use std::path::Path;

use forge_app::FileRemoveService;

#[derive(Default)]
pub struct ForgeFileRemoveService;

#[async_trait::async_trait]
impl FileRemoveService for ForgeFileRemoveService {
    async fn remove(&self, path: &Path) -> anyhow::Result<()> {
        Ok(forge_fs::ForgeFS::remove_file(path).await?)
    }
}
