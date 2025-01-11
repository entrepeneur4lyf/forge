use forge_domain::ToolCallService;
use insta::assert_snapshot;
use tempfile::TempDir;
use tokio::fs;
use crate::test_utils::setup_test_env;

use super::super::{Outline, OutlineInput};

#[tokio::test]
async fn css_outline() {
    let temp_dir = TempDir::new().unwrap();
    let environment = setup_test_env(&temp_dir).await;

    let content = r#"
@media (max-width: 768px) {
    .container {
        width: 100%;
    }
}

@keyframes fade {
    from { opacity: 0; }
    to { opacity: 1; }
}

.header {
    font-size: 2em;
}

#main-content {
    padding: 20px;
}

@import url('other.css');

:root {
    --primary-color: #333;
}

@supports (display: grid) {
    .grid-layout {
        display: grid;
    }
}"#;
    let file_path = temp_dir.path().join("test.css");
    fs::write(&file_path, content).await.unwrap();

    let outline = Outline::new(environment);
    let result = outline
        .call(OutlineInput { path: temp_dir.path().to_string_lossy().to_string() })
        .await
        .unwrap();

    assert_snapshot!("outline_css", result);
}