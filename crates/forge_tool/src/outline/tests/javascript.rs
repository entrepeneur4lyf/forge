use forge_domain::ToolCallService;
use insta::assert_snapshot;
use tempfile::TempDir;
use tokio::fs;

use super::super::{Outline, OutlineInput};
use crate::test_utils::setup_test_env;

#[tokio::test]
async fn javascript_outline() {
    let temp_dir = TempDir::new().unwrap();
    let environment = setup_test_env(&temp_dir).await;

    let content = r#"
// Basic function
function calculateTotal(items) {
    return items.reduce((total, item) => total + item.price, 0);
}

// Arrow function
const processItems = (items) => {
    return items.map(item => item.name);
};

class ShoppingCart {
    constructor() {
        this.items = [];
    }

    // Instance method
    addItem(item) {
        this.items.push(item);
    }

    // Static method
    static getTotalPrice(items) {
        return calculateTotal(items);
    }
}

// Async function
async function fetchItems() {
    return Promise.resolve([]);
}"#;
    let file_path = temp_dir.path().join("test.js");
    fs::write(&file_path, content).await.unwrap();

    let outline = Outline::new(environment);
    let result = outline
        .call(OutlineInput { path: temp_dir.path().to_string_lossy().to_string() })
        .await
        .unwrap();

    assert_snapshot!("outline_javascript", result);
}
