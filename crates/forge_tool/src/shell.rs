use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use forge_domain::{Environment, ToolCallService, ToolDescription};
use forge_tool_macros::ToolDescription;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ShellInput {
    /// The shell command to execute.
    pub command: String,
    /// The working directory where the command should be executed.
    pub cwd: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Execute shell commands with safety checks and validation. This tool provides
/// controlled access to system shell commands while preventing dangerous
/// operations through a comprehensive blacklist and validation system.
#[derive(ToolDescription)]
pub struct Shell {
    blacklist: HashSet<String>,
    environment: Environment,
}

impl Shell {
    pub fn new(environment: Environment) -> Self {
        let mut blacklist = HashSet::new();
        // File System Destruction Commands
        blacklist.insert("rm".to_string());
        blacklist.insert("rmdir".to_string());
        blacklist.insert("del".to_string());

        // Disk/Filesystem Commands
        blacklist.insert("format".to_string());
        blacklist.insert("mkfs".to_string());
        blacklist.insert("dd".to_string());

        // Permission/Ownership Commands
        blacklist.insert("chmod".to_string());
        blacklist.insert("chown".to_string());

        // Privilege Escalation Commands
        blacklist.insert("sudo".to_string());
        blacklist.insert("su".to_string());

        // Code Execution Commands
        blacklist.insert("exec".to_string());
        blacklist.insert("eval".to_string());

        // System Communication Commands
        blacklist.insert("write".to_string());
        blacklist.insert("wall".to_string());

        // System Control Commands
        blacklist.insert("shutdown".to_string());
        blacklist.insert("reboot".to_string());
        blacklist.insert("init".to_string());

        Shell { blacklist, environment }
    }

    async fn validate_command(&self, shell_input: &ShellInput) -> Result<(), String> {
        let paths = self.extract_paths(&shell_input.command);
        for path in paths {
            if !self.validate_path(&path, &self.environment).await? {
                return Err("Access to this path is not allowed".to_string());
            }
        }

        if shell_input.command.is_empty() {
            return Err("Empty command".to_string());
        }

        let base_command = shell_input
            .command
            .split_whitespace()
            .next()
            .ok_or_else(|| "Empty command".to_string())?;

        if self.blacklist.contains(base_command) {
            return Err(format!("Command '{}' is not allowed", base_command));
        }

        Ok(())
    }

    async fn execute_command(&self, command: &str, cwd: PathBuf) -> Result<ShellOutput, String> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", command]);
            c
        };

        cmd.current_dir(cwd);

        let output = cmd.output().await.map_err(|e| e.to_string())?;

        Ok(ShellOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            success: output.status.success(),
        })
    }
}

#[async_trait::async_trait]
impl ToolCallService for Shell {
    type Input = ShellInput;
    type Output = ShellOutput;

    async fn call(&self, input: Self::Input) -> Result<Self::Output, String> {
        self.validate_command(&input).await?;
        self.execute_command(&input.command, input.cwd).await
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    fn create_test_files(dir: &TempDir) -> PathBuf {
        let base = dir.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(base)
            .output()
            .unwrap();

        // Create test files
        fs::write(base.join("normal.txt"), "test").unwrap();
        fs::write(base.join(".hidden.txt"), "hidden").unwrap();

        // Create .gitignore
        fs::write(base.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(base.join("ignored.txt"), "ignored").unwrap();

        base.to_path_buf()
    }

    #[tokio::test]
    async fn test_shell_echo() {
        let environment = Environment::default().cwd(env::current_dir().unwrap());
        let shell = Shell::new(environment);
        let result = shell
            .call(ShellInput {
                command: "echo 'Hello, World!'".to_string(),
                cwd: env::current_dir().unwrap(),
            })
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("Hello, World!"));
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_shell_with_working_directory() {
        let environment = Environment::default().cwd(env::current_dir().unwrap());
        let shell = Shell::new(environment);
        let temp_dir = fs::canonicalize(env::temp_dir()).unwrap();

        let result = shell
            .call(ShellInput {
                command: if cfg!(target_os = "windows") {
                    "cd".to_string()
                } else {
                    "pwd".to_string()
                },
                cwd: temp_dir.clone(),
            })
            .await
            .unwrap();

        assert!(result.success);
        let output_path = PathBuf::from(result.stdout.trim());
        let actual_path = match fs::canonicalize(output_path.clone()) {
            Ok(path) => path,
            Err(_) => output_path,
        };
        let expected_path = temp_dir.as_path();

        assert_eq!(
            actual_path, expected_path,
            "\nExpected path: {:?}\nActual path: {:?}",
            expected_path, actual_path
        );
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_access_hidden_file() {
        let temp = TempDir::new().unwrap();
        let base_path = create_test_files(&temp);
        let environment = Environment::default().cwd(base_path.clone());
        let shell = Shell::new(environment);

        let result = shell
            .call(ShellInput {
                command: format!("cat {}", base_path.join(".hidden.txt").display()),
                cwd: base_path,
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_access_gitignored_file() {
        let temp = TempDir::new().unwrap();
        let base_path = create_test_files(&temp);
        let environment = Environment::default().cwd(base_path.clone());
        let shell = Shell::new(environment);

        let result = shell
            .call(ShellInput {
                command: format!("cat {}", base_path.join("ignored.txt").display()),
                cwd: base_path,
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_access_normal_file() {
        let temp = TempDir::new().unwrap();
        let base_path = create_test_files(&temp);
        let environment = Environment::default().cwd(base_path.clone());
        let shell = Shell::new(environment);

        let result = shell
            .call(ShellInput {
                command: format!("cat {}", base_path.join("normal.txt").display()),
                cwd: base_path,
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().stdout.trim(), "test");
    }

    #[tokio::test]
    async fn test_shell_invalid_command() {
        let environment = Environment::default().cwd(env::current_dir().unwrap());
        let shell = Shell::new(environment);
        let result = shell
            .call(ShellInput {
                command: "nonexistentcommand".to_string(),
                cwd: env::current_dir().unwrap(),
            })
            .await
            .unwrap();

        assert!(!result.success);
        assert!(!result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_shell_blacklisted_command() {
        let environment = Environment::default().cwd(env::current_dir().unwrap());
        let shell = Shell::new(environment);
        let result = shell
            .call(ShellInput {
                command: "rm -rf /".to_string(),
                cwd: env::current_dir().unwrap(),
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }

    #[tokio::test]
    async fn test_shell_empty_command() {
        let environment = Environment::default().cwd(env::current_dir().unwrap());
        let shell = Shell::new(environment);
        let result = shell
            .call(ShellInput { 
                command: "".to_string(), 
                cwd: env::current_dir().unwrap() 
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty command"));
    }

    #[tokio::test]
    async fn test_access_outside_working_directory() {
        let environment = Environment::default().cwd(env::current_dir().unwrap());
        let shell = Shell::new(environment);
        let result = shell
            .call(ShellInput {
                command: "cat /etc/passwd".to_string(),
                cwd: env::current_dir().unwrap(),
            })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not allowed"));
    }
}