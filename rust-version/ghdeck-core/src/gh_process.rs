//! Running GitHub CLI child processes.
//!
//! Every invocation reads stdout and stderr to the end on their own tasks, so
//! no output is lost when the process exits, and stops the child on timeout or
//! cancellation. The [`GhRunner`] trait lets the controller be tested without
//! spawning real processes.

use regex::bytes::Regex;
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep_until, Instant};
use tokio_util::sync::CancellationToken;

pub const STATUS_TIMEOUT: Duration = Duration::from_secs(20);
pub const SWITCH_TIMEOUT: Duration = Duration::from_secs(30);
pub const LOGOUT_TIMEOUT: Duration = Duration::from_secs(30);
pub const API_TIMEOUT: Duration = Duration::from_secs(15);
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
/// GitHub device codes expire after 15 minutes; gh exits by itself then.
pub const LOGIN_TIMEOUT: Duration = Duration::from_secs(16 * 60);

const STDOUT_LIMIT: usize = 1_048_576;
const LOGIN_BUFFER_LIMIT: usize = 4096;
/// After the child exits, how long to keep reading output still in the pipes.
/// A grandchild that inherited a pipe could otherwise keep it open forever.
const DRAIN_GRACE: Duration = Duration::from_secs(1);

/// Variables that would make gh use a token from the environment instead of
/// the stored accounts, or launch a browser command of the caller's choosing.
pub const SANITIZED_ENV_VARS: [&str; 5] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GH_BROWSER",
];

static ANSI_ESCAPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))").expect("ansi regex")
});
static DEVICE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z0-9]{4}-[A-Z0-9]{4}").expect("device code regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Success,
    Failed,
    TimedOut,
    Cancelled,
    SpawnFailed,
}

#[derive(Debug)]
pub struct RunOutput {
    pub status: RunStatus,
    pub stdout: Vec<u8>,
}

impl RunOutput {
    pub fn new(status: RunStatus, stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            stdout: stdout.into(),
        }
    }
}

pub struct RunRequest {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub timeout: Duration,
    /// Stops the child when cancelled.
    pub cancel: Option<CancellationToken>,
    /// Receives the first device authorization code found in the output.
    pub device_code: Option<oneshot::Sender<String>>,
}

impl RunRequest {
    pub fn new(executable: impl Into<PathBuf>, args: &[&str], timeout: Duration) -> Self {
        Self {
            executable: executable.into(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            timeout,
            cancel: None,
            device_code: None,
        }
    }
}

pub trait GhRunner: Send + Sync + 'static {
    fn run(&self, request: RunRequest) -> impl Future<Output = RunOutput> + Send;
}

/// Runs the real GitHub CLI.
pub struct GhCli;

impl GhRunner for GhCli {
    fn run(&self, request: RunRequest) -> impl Future<Output = RunOutput> + Send {
        run_process(request)
    }
}

pub fn sanitize_env(command: &mut std::process::Command) {
    for name in SANITIZED_ENV_VARS {
        command.env_remove(name);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Stdout,
    Stderr,
}

enum Event {
    Chunk(Stream, Vec<u8>),
    StreamsClosed,
    Exited(bool),
    Deadline,
    Cancel,
    DrainExpired,
}

pub async fn run_process(request: RunRequest) -> RunOutput {
    let RunRequest {
        executable,
        args,
        timeout,
        cancel,
        mut device_code,
    } = request;
    let cancel = cancel.unwrap_or_default();

    let mut command = Command::new(&executable);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    sanitize_env(command.as_std_mut());

    let Ok(mut child) = command.spawn() else {
        return RunOutput::new(RunStatus::SpawnFailed, Vec::new());
    };

    let (sender, mut receiver) = mpsc::channel(64);
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(pump(stdout, Stream::Stdout, sender.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(pump(stderr, Stream::Stderr, sender.clone()));
    }
    drop(sender);

    let deadline = Instant::now() + timeout;
    let mut drain_deadline: Option<Instant> = None;
    let mut stdout = Vec::new();
    let mut login_buffer = Vec::new();
    let mut exited: Option<bool> = None;
    let mut streams_open = true;
    let mut stopping = false;
    let mut timed_out = false;
    let mut cancelled = false;

    while streams_open || exited.is_none() {
        let event = tokio::select! {
            biased;
            _ = cancel.cancelled(), if !stopping => Event::Cancel,
            _ = sleep_until(deadline), if !stopping => Event::Deadline,
            message = receiver.recv(), if streams_open => match message {
                Some((stream, data)) => Event::Chunk(stream, data),
                None => Event::StreamsClosed,
            },
            status = child.wait(), if exited.is_none() => {
                Event::Exited(status.map(|status| status.success()).unwrap_or(false))
            }
            _ = sleep_until(drain_deadline.unwrap_or(deadline)), if drain_deadline.is_some() => {
                Event::DrainExpired
            }
        };

        match event {
            Event::Cancel => {
                cancelled = true;
                stopping = true;
                let _ = child.start_kill();
            }
            Event::Deadline => {
                timed_out = true;
                stopping = true;
                let _ = child.start_kill();
            }
            Event::Chunk(stream, data) => {
                if stream == Stream::Stdout {
                    append_limited(&mut stdout, &data, STDOUT_LIMIT);
                }
                if device_code.is_some() && !stopping {
                    append_limited(&mut login_buffer, &data, LOGIN_BUFFER_LIMIT);
                    if let Some(code) = extract_device_code(&login_buffer) {
                        if let Some(sender) = device_code.take() {
                            let _ = sender.send(code);
                        }
                    }
                }
            }
            Event::StreamsClosed => streams_open = false,
            Event::Exited(success) => {
                exited = Some(success);
                drain_deadline = Some(Instant::now() + DRAIN_GRACE);
            }
            Event::DrainExpired => break,
        }
    }

    let status = if cancelled {
        RunStatus::Cancelled
    } else if timed_out {
        RunStatus::TimedOut
    } else if exited == Some(true) {
        RunStatus::Success
    } else {
        RunStatus::Failed
    };
    RunOutput::new(status, stdout)
}

async fn pump<R: AsyncRead + Unpin>(
    mut reader: R,
    stream: Stream,
    sender: mpsc::Sender<(Stream, Vec<u8>)>,
) {
    let mut buffer = vec![0u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                if sender.send((stream, buffer[..read].to_vec())).await.is_err() {
                    break;
                }
            }
        }
    }
}

fn append_limited(buffer: &mut Vec<u8>, data: &[u8], limit: usize) {
    buffer.extend_from_slice(data);
    if buffer.len() > limit {
        let start = buffer.len() - limit;
        buffer.drain(0..start);
    }
}

pub fn extract_device_code(input: &[u8]) -> Option<String> {
    let clean = ANSI_ESCAPE_RE.replace_all(input, b"");
    DEVICE_CODE_RE.find_iter(&clean).find_map(|candidate| {
        let (start, end) = (candidate.start(), candidate.end());
        let valid_left = start == 0 || !is_device_code_character(clean[start - 1]);
        let valid_right = end == clean.len() || !is_device_code_character(clean[end]);
        if !valid_left || !valid_right {
            return None;
        }
        std::str::from_utf8(candidate.as_bytes())
            .ok()
            .map(str::to_string)
    })
}

fn is_device_code_character(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_code_parser_handles_ansi_and_rejects_bad_formats() {
        let mut buffer = b"visit https://example.invalid and enter AB".to_vec();
        buffer.extend_from_slice(b"\x1b[32mCD-EF12\x1b[0m now");
        assert_eq!(extract_device_code(&buffer).as_deref(), Some("ABCD-EF12"));
        assert!(extract_device_code(b"abcd-EF12").is_none());
        assert!(extract_device_code(b"ABC-EF12").is_none());
        assert!(extract_device_code(b"XABCD-EF12Y").is_none());
    }

    #[test]
    fn token_variables_are_removed_not_just_reassigned() {
        let mut command = std::process::Command::new("gh");
        sanitize_env(&mut command);
        let removed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for name in SANITIZED_ENV_VARS {
            assert!(removed.iter().any(|key| key == name), "{name} not removed");
        }
    }

    #[tokio::test]
    async fn missing_executable_reports_spawn_failure() {
        let output = run_process(RunRequest::new(
            r"Z:\definitely\missing\gh.exe",
            &["--version"],
            Duration::from_secs(5),
        ))
        .await;
        assert_eq!(output.status, RunStatus::SpawnFailed);
    }

    #[cfg(windows)]
    mod windows {
        use super::super::*;
        use std::path::Path;

        fn script(dir: &Path, body: &str) -> RunRequest {
            let path = dir.join("fake.cmd");
            std::fs::write(&path, format!("@echo off\r\n{body}\r\n")).unwrap();
            let mut request = RunRequest::new("cmd.exe", &["/d", "/c"], Duration::from_secs(20));
            request.args.push(path.display().to_string());
            request
        }

        #[tokio::test]
        async fn reads_large_output_completely_every_time() {
            let dir = tempfile::tempdir().unwrap();
            let payload: String = (0..4000)
                .map(|index| format!("{{\"login\":\"user{index}\"}},"))
                .collect();
            let data = dir.path().join("status.json");
            std::fs::write(&data, &payload).unwrap();
            for _ in 0..10 {
                let request = script(dir.path(), &format!("type \"{}\"", data.display()));
                let output = run_process(request).await;
                assert_eq!(output.status, RunStatus::Success);
                assert_eq!(output.stdout.len(), payload.len());
            }
        }

        #[tokio::test]
        async fn reports_non_zero_exit_as_failure() {
            let dir = tempfile::tempdir().unwrap();
            let output = run_process(script(dir.path(), "exit /b 3")).await;
            assert_eq!(output.status, RunStatus::Failed);
        }

        #[tokio::test]
        async fn stops_the_child_on_timeout() {
            let dir = tempfile::tempdir().unwrap();
            let mut request = script(dir.path(), "ping -n 5 127.0.0.1 >nul 2>&1");
            request.timeout = Duration::from_millis(300);
            let started = std::time::Instant::now();
            let output = run_process(request).await;
            assert_eq!(output.status, RunStatus::TimedOut);
            assert!(started.elapsed() < Duration::from_secs(10));
        }

        #[tokio::test]
        async fn cancellation_stops_the_child_and_reports_cancelled() {
            let dir = tempfile::tempdir().unwrap();
            let token = CancellationToken::new();
            let mut request = script(dir.path(), "ping -n 5 127.0.0.1 >nul 2>&1");
            request.cancel = Some(token.clone());
            let canceller = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                token.cancel();
            });
            let started = std::time::Instant::now();
            let output = run_process(request).await;
            canceller.await.unwrap();
            assert_eq!(output.status, RunStatus::Cancelled);
            assert!(started.elapsed() < Duration::from_secs(10));
        }

        #[tokio::test]
        async fn reports_device_code_from_stderr_while_running() {
            let dir = tempfile::tempdir().unwrap();
            let mut request = script(
                dir.path(),
                "echo ! First copy your one-time code: AB12-CD34 1>&2\r\nping -n 3 127.0.0.1 >nul 2>&1",
            );
            let (sender, receiver) = oneshot::channel();
            request.device_code = Some(sender);
            let run = tokio::spawn(run_process(request));
            let code = tokio::time::timeout(Duration::from_secs(10), receiver)
                .await
                .expect("code before exit")
                .unwrap();
            assert_eq!(code, "AB12-CD34");
            assert_eq!(run.await.unwrap().status, RunStatus::Success);
        }
    }
}
