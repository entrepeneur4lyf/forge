use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use forge_domain::{Cwd, ToolCallService, ToolDescription};
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
    cwd: Cwd,
}

impl Shell {
    pub fn new(cwd: impl Into<Cwd>) -> Self {
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

        Shell { blacklist, cwd: cwd.into() }
    }
}

impl Shell {
    fn is_path(arg: &str) -> bool {
        arg.starts_with('/')
            || arg.starts_with("./")
            || arg.starts_with("../")
            || PathBuf::from(arg).exists()
    }

    fn extract_paths(command: &str) -> Vec<PathBuf> {
        let args = shlex::split(command).unwrap_or_default();

        args.into_iter()
            .filter(|v| Self::is_path(v))
            .map(PathBuf::from)
            .collect()
    }

    fn validate_command(&self, shell_input: &ShellInput) -> Result<(), String> {
        let cwd = PathBuf::from(self.cwd.as_str()).canonicalize();
        let cwd = cwd.map_err(|e| format!("Unable to validate path: {}", e))?;

        let commands = Self::extract_paths(&shell_input.command);
        let commands = commands
            .into_iter()
            .map(|v| {
                std::fs::canonicalize(v).map_err(|e| format!("Unable to validate path: {}", e))
            })
            .collect::<Result<Vec<_>, _>>()?;

        if commands.into_iter().any(|v| !v.starts_with(&cwd)) {
            return Err(
                "Command is not allowed to access files outside of the current working directory"
                    .to_string(),
            );
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
        self.validate_command(&input)?;
        self.execute_command(&input.command, input.cwd).await
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn test_shell_echo() {
        let shell = Shell::new(".");
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
        let shell = Shell::new(".");
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
    async fn test_shell_invalid_command() {
        let shell = Shell::new(".");
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
        let shell = Shell::new(".");
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
        let shell = Shell::new(".");
        let result = shell
            .call(ShellInput { command: "".to_string(), cwd: env::current_dir().unwrap() })
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Empty command"));
    }

    #[tokio::test]
    async fn test_shell_pwd() {
        let shell = Shell::new(".");
        let current_dir = fs::canonicalize(env::current_dir().unwrap()).unwrap();
        let result = shell
            .call(ShellInput {
                command: if cfg!(target_os = "windows") {
                    "cd".to_string()
                } else {
                    "pwd".to_string()
                },
                cwd: current_dir.clone(),
            })
            .await
            .unwrap();

        assert!(result.success);
        let output_path = PathBuf::from(result.stdout.trim());
        let actual_path = match fs::canonicalize(output_path.clone()) {
            Ok(path) => path,
            Err(_) => output_path,
        };
        assert_eq!(actual_path, current_dir);
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_shell_multiple_commands() {
        let shell = Shell::new(".");
        let result = shell
            .call(ShellInput {
                command: "echo 'first' && echo 'second'".to_string(),
                cwd: env::current_dir().unwrap(),
            })
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("first"));
        assert!(result.stdout.contains("second"));
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn test_shell_with_environment_variables() {
        let shell = Shell::new(".");
        let result = shell
            .call(ShellInput {
                command: "echo $PATH".to_string(),
                cwd: env::current_dir().unwrap(),
            })
            .await
            .unwrap();

        assert!(result.success);
        assert!(!result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn test_description() {
        assert!(Shell::default().description().len() > 100)
    }

    #[tokio::test]
    async fn test_access_too_non_project_files() {
        let shell = Shell::new(".");
        let result = shell
            .call(ShellInput {
                command: "cat /etc/passwd".to_string(),
                cwd: env::current_dir().unwrap(),
            })
            .await;

        assert!(result.is_err());
    }
}
