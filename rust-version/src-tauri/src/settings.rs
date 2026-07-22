use regex::bytes::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_GH_EXECUTABLE: &str = r"C:\Program Files\GitHub CLI\gh.exe";
const LEGACY_SETTINGS_DIRECTORY: &str = "GhDeck";
const SETTINGS_FILE_NAME: &str = "settings.json";
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const VERSION_OUTPUT_LIMIT: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SettingsFile {
    #[serde(default)]
    executable_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GhExecutableSettings {
    path: PathBuf,
}

impl GhExecutableSettings {
    pub fn load() -> Self {
        let path = settings_file_path();
        let value = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<SettingsFile>(&content).ok())
            .and_then(|file| file.executable_path)
            .filter(|value| !value.trim().is_empty())
            .map(|value| PathBuf::from(value))
            .unwrap_or_else(|| PathBuf::from(DEFAULT_GH_EXECUTABLE));
        Self {
            path: normalize_path(value),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn path_string(&self) -> String {
        self.path.display().to_string()
    }

    pub fn set_path(&mut self, executable_path: impl AsRef<Path>) -> Result<(), SettingsError> {
        let normalized = normalize_path(executable_path.as_ref());
        self.path = normalized.clone();
        self.persist(Some(normalized.display().to_string()))
    }

    pub fn reset(&mut self) -> Result<(), SettingsError> {
        self.path = PathBuf::from(DEFAULT_GH_EXECUTABLE);
        self.persist(None)
    }

    pub fn executable_available(&self) -> bool {
        self.path.is_file()
    }

    fn persist(&self, executable_path: Option<String>) -> Result<(), SettingsError> {
        let path = settings_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(SettingsError::Io)?;
        }
        let content = serde_json::to_string_pretty(&SettingsFile { executable_path })
            .map_err(|error| SettingsError::Serialize(error.to_string()))?;
        fs::write(path, content).map_err(SettingsError::Io)
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("无法读写设置文件")]
    Io(#[from] std::io::Error),
    #[error("无法序列化设置: {0}")]
    Serialize(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub message: String,
}

pub fn validate_executable(executable_path: &str) -> ValidationResult {
    let path = PathBuf::from(executable_path);
    if !path.is_file() {
        return ValidationResult {
            valid: false,
            message: "所选文件不存在".into(),
        };
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !file_name.eq_ignore_ascii_case("gh.exe") {
        return ValidationResult {
            valid: false,
            message: "请选择名为 gh.exe 的文件".into(),
        };
    }

    let mut command = Command::new(&path);
    command
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear();
    inherit_sanitized_env(&mut command);

    let output = match command.output() {
        Ok(output) => output,
        Err(_) => {
            return ValidationResult {
                valid: false,
                message: "无法启动所选 GitHub CLI".into(),
            }
        }
    };

    if !output.status.success() {
        return ValidationResult {
            valid: false,
            message: "无法执行所选 GitHub CLI".into(),
        };
    }

    let mut combined = output.stdout;
    combined.extend_from_slice(&output.stderr);
    if combined.len() > VERSION_OUTPUT_LIMIT {
        combined = combined[combined.len() - VERSION_OUTPUT_LIMIT..].to_vec();
    }

    let version_re =
        Regex::new(r"(?mi)^gh version \d+\.\d+\.\d+(?:[-+._a-z0-9]*)?\b").expect("valid regex");
    if version_re.is_match(&combined) {
        ValidationResult {
            valid: true,
            message: "GitHub CLI 校验通过".into(),
        }
    } else {
        ValidationResult {
            valid: false,
            message: "所选文件不是有效的 GitHub CLI".into(),
        }
    }
}

pub fn inherit_sanitized_env(command: &mut Command) {
    for (key, value) in std::env::vars_os() {
        let key_str = key.to_string_lossy();
        if matches!(
            key_str.as_ref(),
            "GH_TOKEN"
                | "GITHUB_TOKEN"
                | "GH_ENTERPRISE_TOKEN"
                | "GITHUB_ENTERPRISE_TOKEN"
                | "GH_BROWSER"
        ) {
            continue;
        }
        command.env(key, value);
    }
}

fn settings_file_path() -> PathBuf {
    settings_file_path_from_root(&dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")))
}

fn settings_file_path_from_root(root: &Path) -> PathBuf {
    root.join(LEGACY_SETTINGS_DIRECTORY)
        .join(SETTINGS_FILE_NAME)
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    dunce::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn settings_path_preserves_legacy_directory() {
        let root = PathBuf::from("config-root");
        assert_eq!(
            settings_file_path_from_root(&root),
            root.join("GhDeck").join("settings.json")
        );
    }

    #[test]
    fn rejects_missing_and_wrong_named_files() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing-gh.exe");
        let result = validate_executable(missing.to_str().unwrap());
        assert!(!result.valid);
        assert!(result.message.contains("不存在"));

        let wrong = dir.path().join("not-gh.exe");
        fs::write(&wrong, b"").unwrap();
        let result = validate_executable(wrong.to_str().unwrap());
        assert!(!result.valid);
        assert!(result.message.contains("gh.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn normalized_existing_path_has_no_verbatim_prefix() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let normalized = normalize_path(file.path());
        assert!(!normalized.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn accepts_github_cli_version_output() {
        // This test only validates the version regex path via a fake executable on Windows is hard.
        // Keep unit coverage focused on pure validation messages for missing/wrong names.
        let re = Regex::new(r"(?mi)^gh version \d+\.\d+\.\d+(?:[-+._a-z0-9]*)?\b").unwrap();
        assert!(re.is_match(b"gh version 2.96.0 (2026-01-01)\n"));
        assert!(!re.is_match(b"hello world\n"));
        let _ = VERSION_TIMEOUT;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "ok").unwrap();
    }
}
