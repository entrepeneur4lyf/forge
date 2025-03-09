use forge_app::CreateDirsService;
use std::path::Path;

#[derive(Default)]
pub struct ForgeCreateDirsService;

#[async_trait::async_trait]
impl CreateDirsService for ForgeCreateDirsService {
    async fn create_dirs(&self, path: &Path) -> anyhow::Result<()> {
        Ok(forge_fs::ForgeFS::create_dir_all(path).await?)
    }
}
