use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

pub fn focused_file_path(json_data: &str) -> Result<String> {
    // Parse the input JSON into a `serde_json::Value`
    let data: Value = serde_json::from_str(json_data)
        .with_context(|| "Failed to parse focused file JSON data")?;

    // Extract the "focus" array, if it exists
    let focus_array = match data.get("focus") {
        Some(Value::Array(arr)) => arr,
        _ => return Err(anyhow!("Invalid JSON format - focus array not found")),
    };

    // Get the first item from "focus", ensuring it's a string
    if let Some(Value::String(item)) = focus_array.first() {
        // The string looks like "file://...::file://..."
        // If you want the second half (the actual file path), split by "::"
        return if let Some(idx) = item.find("::") {
            let second_part = &item[idx + 2..]; // after the "::"
            Ok(second_part
                .strip_prefix("file://")
                .unwrap_or(second_part)
                .to_string())
        } else {
            // If there's no "::", just return the entire string
            Ok(item.strip_prefix("file://").unwrap_or(item).to_string())
        };
    }

    // No first item in the array or it's not a string
    Ok("".to_string())
}

pub fn active_files_path(json_data: &str) -> Result<HashSet<PathBuf>> {
    let parsed: Value = serde_json::from_str(json_data)
        .with_context(|| "Failed to parse VS Code workspace JSON data")?;
    let mut seen_paths = HashSet::new();

    // First get any regular editor paths
    let regular_editor_values = jsonpath_lib::Selector::new()
        .str_path("$..editors[?(@.id=='workbench.editors.files.fileEditorInput')].value")
        .with_context(|| "Invalid JSONPath expression for regular editors")?
        .value(&parsed)
        .select()
        .with_context(|| "Failed to extract regular editor paths")?;

    // Then get diff editor paths
    let diff_editor_values = jsonpath_lib::Selector::new()
        .str_path("$..editors[?(@.id=='workbench.editors.diffEditorInput')].value")
        .with_context(|| "Invalid JSONPath expression for diff editors")?
        .value(&parsed)
        .select()
        .with_context(|| "Failed to extract diff editor paths")?;

    // Process regular editor paths
    for value in regular_editor_values {
        if let Some(value_str) = value.as_str() {
            if let Ok(data) = serde_json::from_str::<Value>(value_str) {
                if let Some(fs_path) = data["resourceJSON"]["fsPath"].as_str() {
                    seen_paths.insert(PathBuf::from(fs_path));
                }
            }
        }
    }

    // Process diff editor paths
    for value in diff_editor_values {
        if let Some(value_str) = value.as_str() {
            if let Ok(value) = serde_json::from_str::<Value>(value_str) {
                // Extract from primary
                if let Some(primary) = value["primarySerialized"].as_str() {
                    if let Ok(data) = serde_json::from_str::<Value>(primary) {
                        if let Some(fs_path) = data["resourceJSON"]["fsPath"].as_str() {
                            seen_paths.insert(PathBuf::from(fs_path));
                        }
                    }
                }
                // Extract from secondary
                if let Some(secondary) = value["secondarySerialized"].as_str() {
                    if let Ok(data) = serde_json::from_str::<Value>(secondary) {
                        if let Some(fs_path) = data["resourceJSON"]["fsPath"].as_str() {
                            seen_paths.insert(PathBuf::from(fs_path));
                        }
                    }
                }
            }
        }
    }

    Ok(seen_paths)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn relative_path(path: &str) -> String {
        // Convert absolute paths in fixtures to relative paths for testing
        path.replace(
            "/Users/tushar/Documents/Projects/code-forge-workspace/feat/impl-list-all-active-files-in-ides/",
            "",
        )
    }

    /// Helper function to extract, normalize and sort paths from workspace data
    fn get_active_files(json_data: &str) -> Vec<String> {
        let mut paths: Vec<_> = active_files_path(json_data)
            .unwrap()
            .into_iter()
            .map(|p| relative_path(p.to_str().unwrap()))
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn test_extract_active_files_1() {
        // Sample file with multiple unique paths
        let json_data = include_str!("fixtures/extract_active_files-1.json");
        let actual = get_active_files(json_data);

        // Expected paths in alphabetical order
        #[rustfmt::skip]
        let expected = vec![
            "crates/forge_app/src/ide.rs",
            "crates/forge_app/src/service/user_prompt.rs",
            "crates/forge_code/src/code.rs",
            "crates/forge_code/src/ide_detection/process.rs",
            "crates/forge_code/src/ide_detection/workspace.rs",
            "crates/forge_code/src/lib.rs",
            "crates/forge_code/src/storage/db.rs",
            "crates/forge_domain/src/ide.rs",
        ];

        assert_eq!(
            actual, expected,
            "Found paths don't match expected paths in fixture 1"
        );
    }

    #[test]
    fn test_extract_active_files_2() {
        // Same paths as test 1 but with duplicates in diff view
        #[rustfmt::skip]
        let expected = vec![
            "crates/forge_app/src/ide.rs",
            "crates/forge_app/src/service/user_prompt.rs",
            "crates/forge_code/src/code.rs",
            "crates/forge_code/src/ide_detection/process.rs",
            "crates/forge_code/src/ide_detection/workspace.rs",
            "crates/forge_code/src/lib.rs",
            "crates/forge_code/src/storage/db.rs",
            "crates/forge_domain/src/ide.rs",
        ];

        let json_data = include_str!("fixtures/extract_active_files-2.json");
        let actual = get_active_files(json_data);
        assert_eq!(
            actual, expected,
            "Found paths don't match expected paths in fixture 2"
        );
    }

    #[test]
    fn test_extract_active_files_3() {
        // Test case with just a single file open
        #[rustfmt::skip]
        let expected = vec![
            "crates/forge_code/src/db.rs",
        ];

        let json_data = include_str!("fixtures/extract_active_files-3.json");
        let actual = get_active_files(json_data);
        assert_eq!(
            actual, expected,
            "Found paths don't match expected paths in fixture 3"
        );
    }

    #[test]
    fn test_extract_active_files_4() {
        // Same content as tests 1 and 2 with diff editors
        #[rustfmt::skip]
        let expected = vec![
            "crates/forge_app/src/ide.rs",
            "crates/forge_app/src/service/user_prompt.rs",
            "crates/forge_code/src/code.rs",
            "crates/forge_code/src/ide_detection/process.rs",
            "crates/forge_code/src/ide_detection/workspace.rs",
            "crates/forge_code/src/lib.rs",
            "crates/forge_code/src/storage/db.rs",
            "crates/forge_domain/src/ide.rs",
        ];

        let json_data = include_str!("fixtures/extract_active_files-4.json");
        let actual = get_active_files(json_data);
        assert_eq!(
            actual, expected,
            "Found paths don't match expected paths in fixture 4"
        );
    }

    #[test]
    fn test_extract_active_files_5() {
        // Similar to test 3, single file test case
        #[rustfmt::skip]
        let expected = vec![
            "crates/forge_code/src/db.rs",
        ];

        let json_data = include_str!("fixtures/extract_active_files-5.json");
        let actual = get_active_files(json_data);
        assert_eq!(
            actual, expected,
            "Found paths don't match expected paths in fixture 5"
        );
    }

    /// Helper function to normalize focused file paths
    fn get_focused_file(json_data: &str) -> String {
        let path = focused_file_path(json_data).unwrap();
        relative_path(&path)
    }

    #[test]
    fn test_extract_focused_file_1() {
        // Empty focus array
        let json_data = include_str!("fixtures/extract_focused_file-1.json");
        let actual = get_focused_file(json_data);
        assert_eq!(actual, "", "Expected empty path for empty focus array");
    }

    #[test]
    fn test_extract_focused_file_2() {
        // Empty focus array (duplicate test)
        let json_data = include_str!("fixtures/extract_focused_file-2.json");
        let actual = get_focused_file(json_data);
        assert_eq!(actual, "", "Expected empty path for empty focus array");
    }

    #[test]
    fn test_extract_focused_file_3() {
        // Focus on sample_focussed_file.json
        let json_data = include_str!("fixtures/extract_focused_file-3.json");
        let actual = get_focused_file(json_data);

        let expected = "crates/forge_code/src/fixtures/sample_focussed_file.json";
        assert_eq!(
            actual, expected,
            "Focused file path doesn't match expected path"
        );
    }

    #[test]
    fn test_extract_focused_file_4() {
        // Same as test 3, focus on sample_focussed_file.json
        let json_data = include_str!("fixtures/extract_focused_file-4.json");
        let actual = get_focused_file(json_data);

        let expected = "crates/forge_code/src/fixtures/sample_focussed_file.json";
        assert_eq!(
            actual, expected,
            "Focused file path doesn't match expected path"
        );
    }
}
