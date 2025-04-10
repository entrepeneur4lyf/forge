#[cfg(test)]
mod tests_mcp {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;
    use forge_domain::{
        Environment, EnvironmentService, LoaderService, McpConfig, McpFsServerConfig, McpHttpServerConfig,
        McpService, Provider, RetryConfig, RunnableService, ToolDefinition, ToolName, Workflow, VERSION,
    };
    use pretty_assertions::assert_eq;
    use rmcp::model::{CallToolResult, Content, Implementation, RawContent, RawTextContent};
    use serde_json::json;
    use uuid::Uuid;

    use crate::infra::{
        FileRemoveService, FsCreateDirsService, FsMetaService, FsReadService, FsSnapshotService,
        FsWriteService, Infrastructure,
    };
    use crate::loader::ForgeLoaderService;
    use crate::mcp::ForgeMcp;

    // Define a minimal workflow for testing
    fn create_test_workflow(with_mcp: bool) -> Workflow {
        if !with_mcp {
            return Workflow::default();
        }

        let mut workflow = Workflow::default();
        let mut mcp_config = McpConfig::default();

        // Add an HTTP server config
        let mut http_servers = HashMap::new();
        http_servers.insert(
            "test_http".to_string(),
            McpHttpServerConfig {
                url: "https://example.com/test".to_string(),
            },
        );
        mcp_config.http = Some(http_servers);

        // Add a FS server config
        let mut fs_servers = HashMap::new();
        fs_servers.insert(
            "test_fs".to_string(),
            McpFsServerConfig {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: None,
            },
        );
        mcp_config.fs = Some(fs_servers);

        workflow.mcp = Some(mcp_config);
        workflow
    }

    // Test infrastructure implementation
    #[derive(Clone)]
    struct TestInfra {
        env_service: TestEnvironmentService,
        fs_meta_service: TestFsMetaService,
        fs_read_service: TestFsReadService,
        fs_remove_service: TestFsRemoveService,
        fs_snapshot_service: TestFsSnapshotService,
        fs_write_service: TestFsWriteService,
        fs_create_dirs_service: TestFsCreateDirsService,
    }

    impl TestInfra {
        fn new() -> Self {
            Self {
                env_service: TestEnvironmentService,
                fs_meta_service: TestFsMetaService,
                fs_read_service: TestFsReadService,
                fs_remove_service: TestFsRemoveService,
                fs_snapshot_service: TestFsSnapshotService,
                fs_write_service: TestFsWriteService,
                fs_create_dirs_service: TestFsCreateDirsService,
            }
        }
    }

    impl Infrastructure for TestInfra {
        type EnvironmentService = TestEnvironmentService;
        type FsMetaService = TestFsMetaService;
        type FsReadService = TestFsReadService;
        type FsRemoveService = TestFsRemoveService;
        type FsSnapshotService = TestFsSnapshotService;
        type FsWriteService = TestFsWriteService;
        type FsCreateDirsService = TestFsCreateDirsService;

        fn environment_service(&self) -> &Self::EnvironmentService {
            &self.env_service
        }

        fn file_meta_service(&self) -> &Self::FsMetaService {
            &self.fs_meta_service
        }

        fn file_read_service(&self) -> &Self::FsReadService {
            &self.fs_read_service
        }

        fn file_remove_service(&self) -> &Self::FsRemoveService {
            &self.fs_remove_service
        }

        fn file_snapshot_service(&self) -> &Self::FsSnapshotService {
            &self.fs_snapshot_service
        }

        fn file_write_service(&self) -> &Self::FsWriteService {
            &self.fs_write_service
        }

        fn create_dirs_service(&self) -> &Self::FsCreateDirsService {
            &self.fs_create_dirs_service
        }
    }

    // Mock service implementations
    #[derive(Clone)]
    struct TestEnvironmentService;

    impl EnvironmentService for TestEnvironmentService {
        fn get_environment(&self) -> Environment {
            Environment {
                os: "test".to_string(),
                pid: 0,
                cwd: PathBuf::from("/test"),
                home: Some(PathBuf::from("/home/test")),
                shell: "test_shell".to_string(),
                base_path: PathBuf::from("/test"),
                provider: Provider::openai("test-key"),
                retry_config: RetryConfig::default(),
            }
        }
    }

    #[derive(Clone)]
    struct TestFsMetaService;

    #[async_trait::async_trait]
    impl FsMetaService for TestFsMetaService {
        async fn is_file(&self, _path: &Path) -> anyhow::Result<bool> {
            Ok(false)
        }

        async fn exists(&self, _path: &Path) -> anyhow::Result<bool> {
            Ok(false)
        }
    }

    #[derive(Clone)]
    struct TestFsReadService;

    #[async_trait::async_trait]
    impl FsReadService for TestFsReadService {
        async fn read(&self, _path: &Path) -> anyhow::Result<Bytes> {
            Ok(Bytes::from_static(b""))
        }
    }

    #[derive(Clone)]
    struct TestFsRemoveService;

    #[async_trait::async_trait]
    impl FileRemoveService for TestFsRemoveService {
        async fn remove(&self, _path: &Path) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct TestFsSnapshotService;

    #[async_trait::async_trait]
    impl FsSnapshotService for TestFsSnapshotService {
        async fn create_snapshot(&self, _file_path: &Path) -> anyhow::Result<forge_snaps::Snapshot> {
            // Create a minimal Snapshot with the right fields
            Ok(forge_snaps::Snapshot {
                id: forge_snaps::SnapshotId::new(),
                path: "test".to_string(),
                timestamp: Duration::from_secs(0),
            })
        }

        async fn undo_snapshot(&self, _file_path: &Path) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct TestFsWriteService;

    #[async_trait::async_trait]
    impl FsWriteService for TestFsWriteService {
        async fn write(&self, _path: &Path, _contents: Bytes) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct TestFsCreateDirsService;

    #[async_trait::async_trait]
    impl FsCreateDirsService for TestFsCreateDirsService {
        async fn create_dirs(&self, _path: &Path) -> anyhow::Result<()> {
            Ok(())
        }
    }

    // Mock implementation for testing
    struct MockLoaderService {
        workflow: Workflow,
    }

    impl MockLoaderService {
        fn with_workflow(workflow: Workflow) -> Self {
            Self { workflow }
        }
    }

    #[async_trait::async_trait]
    impl LoaderService for MockLoaderService {
        async fn load(&self) -> anyhow::Result<Workflow> {
            Ok(self.workflow.clone())
        }
    }

    // Test ForgeMcp with a mock loader
    struct MockForgeMcp {
        loader: MockLoaderService,
    }

    #[async_trait::async_trait]
    impl McpService for MockForgeMcp {
        async fn init_mcp(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn list_tools(&self) -> anyhow::Result<Vec<ToolDefinition>> {
            Ok(vec![ToolDefinition {
                name: ToolName::new("test_tool"),
                description: "Test tool description".to_string(),
                input_schema: serde_json::from_value(serde_json::Value::Object(serde_json::Map::new()))?,
                output_schema: None,
            }])
        }

        async fn stop_all_servers(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_service(&self, _tool_name: &str) -> anyhow::Result<Arc<RunnableService>> {
            Err(anyhow::anyhow!("Not implemented for tests"))
        }

        async fn call_tool(&self, _tool_name: &str, _arguments: serde_json::Value) -> anyhow::Result<CallToolResult> {
            Ok(CallToolResult {
                content: vec![Content {
                    raw: RawContent::Text(RawTextContent {
                        text: "Test result".to_string(),
                    }),
                    annotations: None,
                }],
                is_error: None,
            })
        }
    }

    // Tests

    #[tokio::test]
    async fn test_empty_mcp_config() {
        // Fixture
        let infra = TestInfra::new();
        let loader = ForgeLoaderService::new(Arc::new(infra), None);
        let mcp_service = ForgeMcp::new(loader);

        // Test init with no config
        let result = mcp_service.init_mcp().await;
        assert!(result.is_ok());

        // Verify no tools are listed
        let tools = mcp_service.list_tools().await.unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_client_info() {
        // Verify client info has correct values
        let client_info = ForgeMcp::<TestInfra>::client_info();
        
        assert_eq!(
            client_info.client_info,
            Implementation {
                name: "Forge".to_string(),
                version: VERSION.to_string(),
            }
        );
    }

    #[test]
    fn test_prefix_format_in_tool_name() {
        // Prefix is limited to 10 characters
        let tool_name = "test_tool";
        let server_name = "test_serv"; // 9 chars to be safe
        
        // The actual format is prefix-forgestrip-name
        let expected_prefixed_name = format!("{}-forgestrip-{}", server_name, tool_name);
        
        // Verify the tool name is correctly prefixed
        let prefixed_name = ToolName::prefixed(server_name, tool_name);
        assert_eq!(prefixed_name.to_string(), expected_prefixed_name);
    }

    #[tokio::test]
    async fn test_tool_not_found_error_handling() {
        // Set up mcp service
        let infra = TestInfra::new();
        let loader = ForgeLoaderService::new(Arc::new(infra), None);
        let mcp_service = ForgeMcp::new(loader);
        
        // Test getting a non-existent service
        let result = mcp_service.get_service("non_existent_tool").await;
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "Server not found");
        
        // Test calling a non-existent tool
        let result = mcp_service.call_tool("non_existent_tool", json!({})).await;
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.to_string(), "Server not found");
    }

    #[tokio::test]
    async fn test_mcp_service_workflow() {
        // Create mock loader service with this config
        let workflow = create_test_workflow(true);
        let mock_loader = MockLoaderService::with_workflow(workflow);
        let mock_mcp = MockForgeMcp { loader: mock_loader };
        
        // Verify the mock can list tools
        let tools = mock_mcp.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.to_string(), "test_tool");
        assert_eq!(tools[0].description, "Test tool description");
        
        // Verify we can call tools
        let result = mock_mcp.call_tool("test_tool", json!({})).await.unwrap();
        assert_eq!(result.content[0].raw.as_text().unwrap().text, "Test result");
        assert_eq!(result.is_error, None);
    }

    #[tokio::test]
    async fn test_stop_all_servers_handling() {
        // Create a mock with a test workflow
        let workflow = create_test_workflow(true);
        let mock_loader = MockLoaderService::with_workflow(workflow);
        let mock_mcp = MockForgeMcp { loader: mock_loader };
        
        // Test stopping servers
        let result = mock_mcp.stop_all_servers().await;
        assert!(result.is_ok());
    }
}