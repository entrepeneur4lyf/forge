use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command};
use std::sync::mpsc::{Receiver, Sender};

pub enum ExecutionResult {
    Error,
    Complete,
    Text(String),
}

#[derive(Debug)]
pub struct Executor {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Executor {
    pub fn new(mut command: Command) -> anyhow::Result<Self> {
        let mut child = command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take();
        Ok(Self { child, stdin })
    }

    pub fn execute(&mut self, input: Option<String>) -> Result<String> {
        let stdout = BufReader::new(self.child.stdout.take().context("Failed to open stdout")?);
        let stderr = BufReader::new(self.child.stderr.take().context("Failed to open stderr")?);
        let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
        let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();

        if let Some(ref mut stdin) = self.stdin {
            if let Some(input) = input {
                writeln!(stdin, "{}", input).context("Failed to write to stdin")?;
                stdin.flush().context("Failed to flush stdin")?;
            }
        }
        let _stdout_thread = Self::spawn_reader_thread(stdout, stdout_tx);
        let _stderr_thread = Self::spawn_reader_thread(stderr, stderr_tx);

        let stdout = Self::collect_output(stdout_rx)?;
        let stderr = Self::collect_output(stderr_rx)?;
        Ok(format!("<stdout>{}</stdout>\n<stderr>{}</stderr>", stdout, stderr))
    }

    fn collect_output(rx: Receiver<ExecutionResult>) -> anyhow::Result<String> {
        let mut output = String::new();
        loop {
            match rx.recv() {
                Ok(ExecutionResult::Text(line)) => output.push_str(&line),
                Ok(ExecutionResult::Complete) => break,
                Ok(ExecutionResult::Error) => break,
                Err(_) => break,
            }
        }

        Ok(output)
    }

    pub fn exit(mut self) -> Result<()> {
        drop(self.stdin.take());
        let _ = self.child.wait().context("Failed to wait for process exit")?;
        Ok(())
    }

    fn spawn_reader_thread<R: std::io::Read + Send + 'static>(
        reader: BufReader<R>,
        tx: Sender<ExecutionResult>,
    ) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut line_buffer = String::new();
            let mut reader = reader;

            loop {
                line_buffer.clear();
                match reader.read_line(&mut line_buffer) {
                    Ok(0) => {
                        tx.send(ExecutionResult::Complete).unwrap();
                        break;
                    } // EOF
                    Ok(_) => {
                        tx.send(ExecutionResult::Text(line_buffer.clone())).unwrap();
                        if reader.buffer().is_empty() {
                            tx.send(ExecutionResult::Complete).unwrap();
                        }
                    }
                    Err(_) => {
                        tx.send(ExecutionResult::Error).unwrap();
                        break;
                    }
                }
            }
        })
    }
}
