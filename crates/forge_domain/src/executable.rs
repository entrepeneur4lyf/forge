use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Stdout, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

pub enum ExecutionResult {
    Error,
    Complete,
    Text(String),
}

#[derive(Debug)]
pub struct Executor {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout_rx: Receiver<ExecutionResult>,
}

impl Executor {
    pub fn new(mut command: Command) -> anyhow::Result<Self> {
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().context("Failed to open stdout")?);
        let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _stdout_thread = Self::spawn_reader_thread(stdout, stdout_tx);
        });
        Ok(Self { child, stdin, stdout_rx })
    }

    pub fn execute(&mut self, input: Option<String>, timeout: Option<u64>) -> Result<String> {
        if let Some(ref mut stdin) = self.stdin {
            if let Some(input) = input {
                dbg!(&input);
                writeln!(stdin, "{}", input).context("Failed to write to stdin")?;
                stdin.flush().context("Failed to flush stdin")?;
            }
        }

        // Use a timeout for interactive commands to prevent hanging
        let timeout = timeout.map(Duration::from_secs).unwrap_or(Duration::from_secs(1));
        let stdout = Self::collect_output_with_timeout(&self.stdout_rx, timeout)?;

        Ok(format!("<stdout>{}</stdout>", stdout))
    }

    // We don't need this function anymore.
    fn collect_output_with_timeout(rx: &Receiver<ExecutionResult>, timeout: Duration) -> anyhow::Result<String> {
        let mut output = String::new();
        let mut received_data = false;

        // Keep receiving until timeout with no data
        loop {
            match rx.recv_timeout(timeout) {
                Ok(ExecutionResult::Text(line)) => {
                    output.push_str(&line);
                    received_data = true;
                }
                Ok(ExecutionResult::Complete) => {
                    break;
                }
                Ok(ExecutionResult::Error) => break,
                Err(_) => {
                    // If we've received data and then hit a timeout, assume the interactive
                    // command is waiting for more input, so we can return
                    if received_data {
                        break;
                    } else {
                        if output.is_empty() {
                            break;
                        }
                    }
                }
            }
        }

        Ok(output)
    }

    pub fn exit(&mut self) -> Result<()> {
        drop(self.stdin.take());
        let _ = self.child.wait().context("Failed to wait for process exit")?;
        Ok(())
    }

    fn spawn_reader_thread(
        mut stdout: BufReader<ChildStdout>,
        tx: Sender<ExecutionResult>,
    ) {
        let mut line_buffer = String::new();
        loop {
            line_buffer.clear();
            match stdout.read_line(&mut line_buffer) {
                Ok(0) => {
                    // EOF reached - send Complete and break
                    let _ = tx.send(ExecutionResult::Complete);
                    break;
                }
                Ok(_) => {
                    dbg!(&line_buffer);
                    // Successfully read data, send it if channel is still open
                    if tx.send(ExecutionResult::Text(line_buffer.clone())).is_err() {
                        // Receiver was dropped, exit the thread
                        break;
                    }
                    if stdout.buffer().is_empty() {
                        // If buffer is empty, we've read all available data
                        let _ = tx.send(ExecutionResult::Complete);
                        // break;
                    }
                }
                Err(_) => {
                    // Error reading data, notify receiver if still listening
                    let _ = tx.send(ExecutionResult::Error);
                    break;
                }
            }
        }
    }
}