use std::path::{Path, PathBuf};

use forge_walker::Walker;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::Environment;

/// A trait that defines the interface for tool implementations.
/// Each tool must implement this trait to be usable by the forge system.
#[async_trait::async_trait]
pub trait ToolCallService
where
    Self: Send + Sync,
{
    type Input: DeserializeOwned;
    type Output: Serialize;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String>;

    /// Validates if a given path is allowed within the context of the current
    /// working directory and is neither hidden nor gitignored. This default
    /// implementation can be used by all tools that need path validation.
    async fn validate_path(&self, path: &Path, environment: &Environment) -> Result<bool, String> {
        let cwd = environment.cwd.as_path();

        // Ensure path is within working directory
        let canonical_path =
            std::fs::canonicalize(path).map_err(|e| format!("Unable to validate path: {}", e))?;
        let canonical_base = std::fs::canonicalize(cwd)
            .map_err(|e| format!("Unable to validate base path: {}", e))?;

        if !canonical_path.starts_with(&canonical_base) {
            return Ok(false);
        }

        // Get the list of allowed files from forge_walker (automatically handles hidden
        // and gitignored files)
        let walker = Walker::new(cwd.to_path_buf());
        let allowed_files = walker
            .get()
            .await
            .map_err(|e| format!("Failed to walk directory: {}", e))?;

        // Convert the input path to be relative to base_path
        let relative_path = canonical_path
            .strip_prefix(&canonical_base)
            .map_err(|_| "Failed to get relative path".to_string())?
            .to_string_lossy()
            .to_string();

        // If the file is in the allowed files list, it's not hidden or gitignored
        let is_allowed = allowed_files.iter().any(|f| f.path == relative_path);
        Ok(is_allowed)
    }

    /// Helper method to determine if a string might be a path
    fn is_path(&self, arg: &str) -> bool {
        arg.starts_with('/')
            || arg.starts_with("./")
            || arg.starts_with("../")
            || PathBuf::from(arg).exists()
    }

    /// Extract potential paths from a string
    fn extract_paths(&self, text: &str) -> Vec<PathBuf> {
        text.split_whitespace()
            .filter(|v| self.is_path(v))
            .map(PathBuf::from)
            .collect()
    }
}
