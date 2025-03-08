use std::sync::Arc;

use forge_app::{EnvironmentService, Infrastructure};
use forge_snaps::ForgeSnapshotService;

use crate::embedding::OpenAIEmbeddingService;
use crate::env::ForgeEnvironmentService;
use crate::file_meta::ForgeFileMetaService;
use crate::file_read::ForgeFileReadService;
use crate::file_write::ForgeFileWriteService;
use crate::qdrant::QdrantVectorIndex;

pub struct Resolved;
pub struct UnResolved;

pub struct ForgeInfra<T> {
    file_read_service: ForgeFileReadService,
    file_write_service: ForgeFileWriteService,
    environment_service: ForgeEnvironmentService,
    information_repo: QdrantVectorIndex,
    embedding_service: OpenAIEmbeddingService,
    snap_service: Arc<ForgeSnapshotService>,
    file_exists_service: ForgeFileMetaService,

    _marker: std::marker::PhantomData<T>,
}

impl ForgeInfra<UnResolved> {
    pub fn new(restricted: bool) -> Self {
        let environment_service = ForgeEnvironmentService::new(restricted);
        let env = environment_service.get_environment();
        let snap_service = Arc::new(ForgeSnapshotService::default());
        Self {
            file_read_service: ForgeFileReadService::new(),
            file_write_service: ForgeFileWriteService::new(snap_service.clone()),
            environment_service,
            information_repo: QdrantVectorIndex::new(env.clone(), "user_feedback"),
            embedding_service: OpenAIEmbeddingService::new(env),
            snap_service,
            file_exists_service: ForgeFileMetaService,
            _marker: Default::default(),
        }
    }

    pub fn transform(self, snap_service: Arc<ForgeSnapshotService>) -> ForgeInfra<Resolved> {
        ForgeInfra {
            file_read_service: self.file_read_service,
            file_write_service: ForgeFileWriteService::new(snap_service.clone()),
            environment_service: self.environment_service,
            information_repo: self.information_repo,
            embedding_service: self.embedding_service,
            snap_service,
            file_exists_service: ForgeFileMetaService,
            _marker: Default::default(),
        }
    }

    pub fn env(&self) -> &ForgeEnvironmentService {
        &self.environment_service
    }
}

impl Infrastructure for ForgeInfra<Resolved> {
    type EnvironmentService = ForgeEnvironmentService;
    type FileReadService = ForgeFileReadService;
    type FileWriteService = ForgeFileWriteService;
    type VectorIndex = QdrantVectorIndex;
    type EmbeddingService = OpenAIEmbeddingService;
    type FileSnapshotService = ForgeSnapshotService;
    type FileMetaService = ForgeFileMetaService;

    fn environment_service(&self) -> &Self::EnvironmentService {
        &self.environment_service
    }

    fn file_read_service(&self) -> &Self::FileReadService {
        &self.file_read_service
    }

    fn vector_index(&self) -> &Self::VectorIndex {
        &self.information_repo
    }

    fn embedding_service(&self) -> &Self::EmbeddingService {
        &self.embedding_service
    }

    fn file_write_service(&self) -> &Self::FileWriteService {
        &self.file_write_service
    }

    fn file_snapshot_service(&self) -> &Self::FileSnapshotService {
        &self.snap_service
    }

    fn file_meta_service(&self) -> &Self::FileMetaService {
        &self.file_exists_service
    }
}
