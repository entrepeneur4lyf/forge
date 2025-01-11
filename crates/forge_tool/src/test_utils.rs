use forge_domain::Environment;
use tempfile::TempDir;
use tokio::fs;

/// Sets up a test environment with a git repository and .gitignore
/// Returns the Environment configured with the temp directory as CWD
pub async fn setup_test_env(dir: &TempDir) -> Environment {
    // Initialize git repo for gitignore tests
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Create .gitignore
    fs::write(dir.path().join(".gitignore"), "ignored.txt\n")
        .await
        .unwrap();
    
    Environment::default().cwd(dir.path().to_path_buf())
}

// Other test utilities can be added here as needed