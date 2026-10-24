use crate::gh_process::{run_process, RunRequest, RunStatus, VERSION_TIMEOUT};
use regex::bytes::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub const DEFAULT_GH_EXECUTABLE: &str = r"C:\Program Files\GitHub CLI\gh.exe";
/// Kept from the GhDeck days so existing settings survive the rename.
const LEGACY_SETTINGS_DIRECTORY: &str = "GhDeck";
const SETTINGS_FILE_NAME: &str = "settings.json";

static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?mi)^gh version \d+\.\d+\.\d+(?:[-+._a-z0-9]*)?\b").expect("version regex")
});

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SettingsFile {
    #[serde(default)]
    executable_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GhExecutableSettings {
    path: PathBuf,
    file: PathBuf,
}

impl GhExecutableSettings {
    pub fn load() -> Self {
        Self::load_from(app_config_dir().join(SETTINGS_FILE_NAME))
    }

    pub fn load_from(file: PathBuf) -> Self {
        let path = fs::read_to_string(&file)
            .ok()
            .and_then(|content| serde_json::from_str::<SettingsFile>(&content).ok())
            .and_then(|settings| settings.executable_path)
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_GH_EXECUTABLE));
        Self {
            path: normalize_path(path),
            file,
        }
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn path_string(&self) -> String {
        self.path.display().to_string()
    }

    pub fn set_path(&mut self, executable_path: impl AsRef<Path>) -> Result<(), String> {
        let normalized = normalize_path(executable_path.as_ref());
        self.persist(Some(normalized.display().to_string()))?;
        self.path = normalized;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), String> {
        self.persist(None)?;
        self.path = PathBuf::from(DEFAULT_GH_EXECUTABLE);
        Ok(())
    }

    pub fn executable_available(&self) -> bool {
        self.path.is_file()
    }

    fn persist(&self, executable_path: Option<String>) -> Result<(), String> {
        let content = serde_json::to_string_pretty(&SettingsFile { executable_path })
            .map_err(|_| "无法保存设置".to_string())?;
        write_file(&self.file, &content)
    }
}

pub fn app_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(LEGACY_SETTINGS_DIRECTORY)
}

/// Writes through a temporary file so a crash never leaves half a file.
pub fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "无法创建设置目录".to_string())?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, content).map_err(|_| "无法写入设置文件".to_string())?;
    fs::rename(&temporary, path).map_err(|_| "无法写入设置文件".to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub message: String,
}

impl ValidationResult {
    fn invalid(message: &str) -> Self {
        Self {
            valid: false,
            message: message.into(),
        }
    }
}

pub async fn validate_executable(executable_path: &str) -> ValidationResult {
    let path = PathBuf::from(executable_path.trim());
    if !path.is_file() {
        return ValidationResult::invalid("所选文件不存在");
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !file_name.eq_ignore_ascii_case("gh.exe") {
        return ValidationResult::invalid("请选择名为 gh.exe 的文件");
    }

    let output = run_process(RunRequest::new(path, &["--version"], VERSION_TIMEOUT)).await;
    match output.status {
        RunStatus::Success if VERSION_RE.is_match(&output.stdout) => ValidationResult {
            valid: true,
            message: "GitHub CLI 校验通过".into(),
        },
        RunStatus::Success => ValidationResult::invalid("所选文件不是有效的 GitHub CLI"),
        RunStatus::SpawnFailed => ValidationResult::invalid("无法启动所选 GitHub CLI"),
        RunStatus::TimedOut => ValidationResult::invalid("所选 GitHub CLI 没有响应"),
        RunStatus::Failed | RunStatus::Cancelled => {
            ValidationResult::invalid("无法执行所选 GitHub CLI")
        }
    }
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
    use tempfile::tempdir;

    #[test]
    fn settings_directory_keeps_legacy_name() {
        assert!(app_config_dir().ends_with("GhDeck"));
    }

    #[test]
    fn set_reset_and_reload_round_trip() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let executable = dir.path().join("gh.exe");
        fs::write(&executable, b"").unwrap();

        let mut settings = GhExecutableSettings::load_from(file.clone());
        assert_eq!(settings.path(), PathBuf::from(DEFAULT_GH_EXECUTABLE));
        settings.set_path(&executable).unwrap();
        let reloaded = GhExecutableSettings::load_from(file.clone());
        assert_eq!(reloaded.path(), normalize_path(&executable));

        settings.reset().unwrap();
        let reloaded = GhExecutableSettings::load_from(file);
        assert_eq!(reloaded.path(), PathBuf::from(DEFAULT_GH_EXECUTABLE));
    }

    #[tokio::test]
    async fn rejects_missing_and_wrong_named_files() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing-gh.exe");
        let result = validate_executable(missing.to_str().unwrap()).await;
        assert!(!result.valid);
        assert!(result.message.contains("不存在"));

        let wrong = dir.path().join("not-gh.exe");
        fs::write(&wrong, b"").unwrap();
        let result = validate_executable(wrong.to_str().unwrap()).await;
        assert!(!result.valid);
        assert!(result.message.contains("gh.exe"));
    }

    #[test]
    fn version_pattern_matches_gh_output_only() {
        assert!(VERSION_RE.is_match(b"gh version 2.96.0 (2026-07-02)\nhttps://github.com/cli/cli\n"));
        assert!(!VERSION_RE.is_match(b"hello world\n"));
    }

    #[cfg(windows)]
    #[test]
    fn normalized_existing_path_has_no_verbatim_prefix() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let normalized = normalize_path(file.path());
        assert!(!normalized.to_string_lossy().starts_with(r"\\?\"));
    }
}
