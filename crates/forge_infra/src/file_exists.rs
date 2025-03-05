use std::path::Path;

use anyhow::Result;
use forge_app::FileExist;

pub struct FileExistService;
#[async_trait::async_trait]
impl FileExist for FileExistService {
    async fn exist(&self, path: &Path) -> Result<bool> {
        Ok(forge_fs::ForgeFS::exists(path))
    }
}
