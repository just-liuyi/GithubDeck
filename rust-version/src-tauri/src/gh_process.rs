use crate::settings::{inherit_sanitized_env, GhExecutableSettings};
use once_cell::sync::Lazy;
use regex::bytes::Regex;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Instant};

const STATUS_TIMEOUT: Duration = Duration::from_secs(15);
const SWITCH_TIMEOUT: Duration = Duration::from_secs(30);
const LOGOUT_TIMEOUT: Duration = Duration::from_secs(30);
const LOGIN_BUFFER_LIMIT: usize = 4096;
const STATUS_OUTPUT_LIMIT: usize = 1_048_576;

static ANSI_ESCAPE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))").expect("ansi regex")
});
static DEVICE_CODE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Z0-9]{4}-[A-Z0-9]{4}").expect("device code regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Status,
    Switch,
    Logout,
    Login,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Switch => "switch",
            Self::Logout => "logout",
            Self::Login => "login",
        }
    }

    fn timeout(self) -> Option<Duration> {
        match self {
            Self::Status => Some(STATUS_TIMEOUT),
            Self::Switch => Some(SWITCH_TIMEOUT),
            Self::Logout => Some(LOGOUT_TIMEOUT),
            Self::Login => None,
        }
    }
}

#[derive(Debug)]
struct ProcessState {
    child: Option<Child>,
    operation: Option<Operation>,
    cancelled: bool,
    login_buffer: Vec<u8>,
    status_output: Vec<u8>,
    code_reported: bool,
    browser_open_confirmed: bool,
}

impl Default for ProcessState {
    fn default() -> Self {
        Self {
            child: None,
            operation: None,
            cancelled: false,
            login_buffer: Vec::new(),
            status_output: Vec::new(),
            code_reported: false,
            browser_open_confirmed: false,
        }
    }
}

#[derive(Clone)]
pub struct GhProcess {
    settings: Arc<Mutex<GhExecutableSettings>>,
    state: Arc<Mutex<ProcessState>>,
}

impl GhProcess {
    pub fn new(settings: GhExecutableSettings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
            state: Arc::new(Mutex::new(ProcessState::default())),
        }
    }

    pub async fn settings_path(&self) -> String {
        self.settings.lock().await.path_string()
    }

    pub async fn set_settings_path(&self, path: &str) -> Result<(), String> {
        self.settings
            .lock()
            .await
            .set_path(path)
            .map_err(|error| error.to_string())
    }

    pub async fn reset_settings_path(&self) -> Result<(), String> {
        self.settings
            .lock()
            .await
            .reset()
            .map_err(|error| error.to_string())
    }

    pub async fn executable_available(&self) -> bool {
        self.settings.lock().await.executable_available()
    }

    pub async fn busy(&self) -> bool {
        self.state.lock().await.child.is_some()
    }

    pub async fn refresh(&self, app: AppHandle) -> Result<(), String> {
        self.start(app, Operation::Status, &["auth", "status", "--json", "hosts"])
            .await
    }

    pub async fn switch(&self, app: AppHandle, host: &str, login: &str) -> Result<(), String> {
        self.start(
            app,
            Operation::Switch,
            &["auth", "switch", "--hostname", host, "--user", login],
        )
        .await
    }

    pub async fn logout(&self, app: AppHandle, host: &str, login: &str) -> Result<(), String> {
        self.start(
            app,
            Operation::Logout,
            &["auth", "logout", "--hostname", host, "--user", login],
        )
        .await
    }

    pub async fn login(&self, app: AppHandle) -> Result<(), String> {
        self.start(
            app,
            Operation::Login,
            &[
                "auth",
                "login",
                "--hostname",
                "github.com",
                "--git-protocol",
                "https",
                "--web",
            ],
        )
        .await
    }

    pub async fn cancel(&self) -> bool {
        let mut state = self.state.lock().await;
        if state.child.is_none() {
            return false;
        }
        state.cancelled = true;
        state.login_buffer.clear();
        state.status_output.clear();
        state.code_reported = false;
        state.browser_open_confirmed = false;
        if let Some(child) = state.child.as_mut() {
            let _ = child.start_kill();
        }
        true
    }

    pub async fn shutdown(&self) {
        let mut state = self.state.lock().await;
        state.cancelled = true;
        if let Some(mut child) = state.child.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        *state = ProcessState::default();
    }

    async fn start(
        &self,
        app: AppHandle,
        operation: Operation,
        arguments: &[&str],
    ) -> Result<(), String> {
        {
            let state = self.state.lock().await;
            if state.child.is_some() {
                return Err("当前有操作正在进行".into());
            }
        }

        let executable = {
            let settings = self.settings.lock().await;
            let path = settings.path();
            if !path.is_file() {
                return Err("未找到 GitHub CLI，请在设置中指定有效的 gh.exe".into());
            }
            path
        };

        let mut command = Command::new(&executable);
        command
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        inherit_sanitized_env(command.as_std_mut());

        let child = command
            .spawn()
            .map_err(|_| "无法启动 GitHub CLI".to_string())?;

        {
            let mut state = self.state.lock().await;
            state.child = Some(child);
            state.operation = Some(operation);
            state.cancelled = false;
            state.login_buffer.clear();
            state.status_output.clear();
            state.code_reported = false;
            state.browser_open_confirmed = false;
        }

        let process = self.clone();
        tokio::spawn(async move {
            process.drive_operation(app, operation).await;
        });
        Ok(())
    }

    async fn drive_operation(&self, app: AppHandle, operation: Operation) {
        let started = Instant::now();
        let mut stdout_buf = [0u8; 1024];
        let mut stderr_buf = [0u8; 1024];

        loop {
            if let Some(limit) = operation.timeout() {
                if started.elapsed() >= limit {
                    let _ = self.cancel().await;
                }
            }

            let mut state = self.state.lock().await;
            if state.child.is_none() {
                return;
            }

            let mut made_progress = false;
            let mut stdout_chunk: Option<Vec<u8>> = None;
            let mut stderr_chunk: Option<Vec<u8>> = None;

            if let Some(child) = state.child.as_mut() {
                if let Some(stdout) = child.stdout.as_mut() {
                    match timeout(Duration::from_millis(50), stdout.read(&mut stdout_buf)).await {
                        Ok(Ok(0)) => {}
                        Ok(Ok(n)) => {
                            made_progress = true;
                            stdout_chunk = Some(stdout_buf[..n].to_vec());
                        }
                        Ok(Err(_)) | Err(_) => {}
                    }
                }
            }

            if let Some(chunk) = stdout_chunk.as_deref() {
                if operation == Operation::Login {
                    Self::consume_login_output(&mut state, chunk, &app);
                } else if operation == Operation::Status {
                    append_limited(&mut state.status_output, chunk, STATUS_OUTPUT_LIMIT);
                }
            }

            if let Some(child) = state.child.as_mut() {
                if let Some(stderr) = child.stderr.as_mut() {
                    match timeout(Duration::from_millis(50), stderr.read(&mut stderr_buf)).await {
                        Ok(Ok(0)) => {}
                        Ok(Ok(n)) => {
                            made_progress = true;
                            stderr_chunk = Some(stderr_buf[..n].to_vec());
                        }
                        Ok(Err(_)) | Err(_) => {}
                    }
                }
            }

            if let Some(chunk) = stderr_chunk.as_deref() {
                if operation == Operation::Login {
                    Self::consume_login_output(&mut state, chunk, &app);
                }
            }

            if operation == Operation::Login
                && state.code_reported
                && !state.browser_open_confirmed
                && !state.cancelled
            {
                let browser_open_confirmed = if let Some(child) = state.child.as_mut() {
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(b"\n").await.is_ok()
                    } else {
                        false
                    }
                } else {
                    false
                };
                if browser_open_confirmed {
                    state.browser_open_confirmed = true;
                }
            }

            let wait_result = match state.child.as_mut() {
                Some(child) => child.try_wait(),
                None => return,
            };

            match wait_result {
                Ok(Some(status)) => {
                    let cancelled = state.cancelled;
                    let output = if operation == Operation::Status {
                        state.status_output.clone()
                    } else {
                        Vec::new()
                    };
                    state.child = None;
                    state.operation = None;
                    state.login_buffer.clear();
                    state.status_output.clear();
                    state.code_reported = false;
                    state.browser_open_confirmed = false;
                    let success = status.success() && !cancelled;
                    drop(state);
                    let _ = app.emit(
                        "gh-finished",
                        ProcessFinishedEvent {
                            operation: operation.as_str().to_string(),
                            success,
                            output: String::from_utf8_lossy(&output).into_owned(),
                        },
                    );
                    return;
                }
                Ok(None) => {}
                Err(_) => {
                    state.child = None;
                    state.operation = None;
                    drop(state);
                    let _ = app.emit(
                        "gh-finished",
                        ProcessFinishedEvent {
                            operation: operation.as_str().to_string(),
                            success: false,
                            output: String::new(),
                        },
                    );
                    return;
                }
            }

            drop(state);
            if !made_progress {
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
        }
    }

    fn consume_login_output(state: &mut ProcessState, data: &[u8], app: &AppHandle) {
        if state.cancelled || state.code_reported || data.is_empty() {
            return;
        }
        append_limited(&mut state.login_buffer, data, LOGIN_BUFFER_LIMIT);
        let clean = ANSI_ESCAPE_RE.replace_all(&state.login_buffer, b"");
        if let Some(code) = find_device_code(&clean) {
            state.code_reported = true;
            let _ = app.emit(
                "authorization-code",
                AuthorizationCodeEvent { code },
            );
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

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessFinishedEvent {
    operation: String,
    success: bool,
    output: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizationCodeEvent {
    code: String,
}

pub fn logout_arguments(host: &str, login: &str) -> Vec<String> {
    vec![
        "auth".into(),
        "logout".into(),
        "--hostname".into(),
        host.into(),
        "--user".into(),
        login.into(),
    ]
}

pub fn login_arguments() -> Vec<String> {
    vec![
        "auth".into(),
        "login".into(),
        "--hostname".into(),
        "github.com".into(),
        "--git-protocol".into(),
        "https".into(),
        "--web".into(),
    ]
}

fn find_device_code(input: &[u8]) -> Option<String> {
    DEVICE_CODE_RE.find_iter(input).find_map(|candidate| {
        let start = candidate.start();
        let end = candidate.end();
        let valid_left = start == 0 || !is_device_code_character(input[start - 1]);
        let valid_right = end == input.len() || !is_device_code_character(input[end]);
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

pub fn extract_device_code(input: &[u8]) -> Option<String> {
    let clean = ANSI_ESCAPE_RE.replace_all(input, b"");
    find_device_code(&clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logout_uses_exact_account_arguments() {
        assert_eq!(
            logout_arguments("github.com", "alice"),
            vec![
                "auth",
                "logout",
                "--hostname",
                "github.com",
                "--user",
                "alice"
            ]
        );
    }

    #[test]
    fn login_uses_web_flow_arguments() {
        assert_eq!(
            login_arguments(),
            vec![
                "auth",
                "login",
                "--hostname",
                "github.com",
                "--git-protocol",
                "https",
                "--web"
            ]
        );
    }

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
    fn executable_path_type_is_pathbuf() {
        let path = std::path::PathBuf::from(r"C:\Program Files\GitHub CLI\gh.exe");
        assert!(path.to_string_lossy().contains("gh.exe"));
    }
}
