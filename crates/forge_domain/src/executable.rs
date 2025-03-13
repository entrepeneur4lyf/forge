use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{timeout, Duration};

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
    pub stderr_rx: Receiver<ExecutionResult>,
}

impl Executor {
    pub async fn new(mut command: Command) -> anyhow::Result<Self> {
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to spawn command")?;

        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().context("Failed to open stdout")?);
        let stderr = BufReader::new(child.stderr.take().context("Failed to open stderr")?);

        let (stdout_tx, stdout_rx) = tokio::sync::mpsc::channel(100);
        let (stderr_tx, stderr_rx) = tokio::sync::mpsc::channel(100);

        // Spawn tasks to handle stdout and stderr
        Self::spawn_reader_task(stdout, stdout_tx);
        Self::spawn_reader_task(stderr, stderr_tx);

        Ok(Self { child, stdin, stdout_rx, stderr_rx })
    }

    pub async fn execute(
        &mut self,
        input: Option<String>,
        timeout_secs: Option<u64>,
    ) -> Result<String> {
        if let Some(ref mut stdin) = self.stdin {
            if let Some(input) = input {
                stdin
                    .write_all(format!("{}\n", input).as_bytes())
                    .await
                    .context("Failed to write to stdin")?;
                stdin.flush().await.context("Failed to flush stdin")?;
            }
        }

        // Use a timeout for interactive commands to prevent hanging
        let timeout_duration = timeout_secs
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(1));

        let stdout =
            Self::collect_output_with_timeout(&mut self.stdout_rx, timeout_duration).await?;
        let stderr =
            Self::collect_output_with_timeout(&mut self.stderr_rx, timeout_duration).await?;

        let resp = format!("<stdout>{}</stdout><stderr>{}</stderr>", stdout, stderr);

        Ok(resp)
    }

    async fn collect_output_with_timeout(
        rx: &mut Receiver<ExecutionResult>,
        timeout_duration: Duration,
    ) -> anyhow::Result<String> {
        let mut output = String::new();

        // Keep receiving until timeout with no data
        loop {
            match timeout(timeout_duration, rx.recv()).await {
                Ok(Some(ExecutionResult::Text(line))) => {
                    output.push_str(&line);
                }
                Ok(Some(ExecutionResult::Complete)) => {
                    break;
                }
                Ok(Some(ExecutionResult::Error)) => break,
                Ok(None) => {
                    // Channel closed
                    break;
                }
                Err(_) => {
                    // Timeout occurred
                    break;
                }
            }
        }

        Ok(output)
    }

    pub async fn exit(&mut self) -> Result<()> {
        drop(self.stdin.take());
        let _status = self
            .child
            .wait()
            .await
            .context("Failed to wait for process exit")?;
        Ok(())
    }

    fn spawn_reader_task<R>(mut reader: BufReader<R>, tx: Sender<ExecutionResult>)
    where
        R: AsyncReadExt + Unpin + Send + 'static,
    {
        tokio::spawn(async move {
            let mut line_buffer = String::new();
            loop {
                line_buffer.clear();
                match reader.read_line(&mut line_buffer).await {
                    Ok(0) => {
                        // EOF reached - send Complete and break
                        let _ = tx.send(ExecutionResult::Complete).await;
                        break;
                    }
                    Ok(_) => {
                        // Successfully read data, send it if channel is still open
                        if tx
                            .send(ExecutionResult::Text(line_buffer.clone()))
                            .await
                            .is_err()
                        {
                            // Receiver was dropped, exit the task
                            break;
                        }

                        // Check if there's more data available
                        // Note: tokio's BufReader doesn't expose buffer contents, so we use a
                        // slightly different approach here than the
                        // original
                        if tx.send(ExecutionResult::Complete).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        // Error reading data, notify receiver if still listening
                        let _ = tx.send(ExecutionResult::Error).await;
                        break;
                    }
                }
            }
        });
    }
}
