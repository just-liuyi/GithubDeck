//! Installed browsers and their user profiles, so an account can be tied to
//! the browser (and profile) where that GitHub account is signed in.
//!
//! Browsers come from the Windows `StartMenuInternet` registration. Chromium
//! browsers list their profiles in `Local State`; Firefox lists them in
//! `profiles.ini`. Targets are identified by registry key plus profile, which
//! stays stable across browser updates.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserTarget {
    pub id: String,
    pub label: String,
    pub executable: PathBuf,
    /// Arguments placed before the URL, such as the profile selection.
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledBrowser {
    pub key: String,
    pub name: String,
    pub executable: PathBuf,
}

enum Family {
    Chromium(&'static [&'static str]),
    Firefox,
    Other,
}

fn family(executable: &Path) -> Family {
    let name = executable
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match name.as_str() {
        "chrome.exe" => Family::Chromium(&["Google", "Chrome", "User Data"]),
        "msedge.exe" => Family::Chromium(&["Microsoft", "Edge", "User Data"]),
        "brave.exe" => Family::Chromium(&["BraveSoftware", "Brave-Browser", "User Data"]),
        "vivaldi.exe" => Family::Chromium(&["Vivaldi", "User Data"]),
        "chromium.exe" => Family::Chromium(&["Chromium", "User Data"]),
        "firefox.exe" => Family::Firefox,
        _ => Family::Other,
    }
}

pub fn detect() -> Vec<BrowserTarget> {
    expand_targets(
        &installed_browsers(),
        dirs::data_local_dir().as_deref(),
        dirs::data_dir().as_deref(),
    )
}

pub fn expand_targets(
    installed: &[InstalledBrowser],
    local_app_data: Option<&Path>,
    roaming_app_data: Option<&Path>,
) -> Vec<BrowserTarget> {
    let mut targets = Vec::new();
    for browser in installed {
        let plain = BrowserTarget {
            id: format!("{}|", browser.key),
            label: browser.name.clone(),
            executable: browser.executable.clone(),
            args: Vec::new(),
        };
        match family(&browser.executable) {
            Family::Chromium(relative) => {
                let profiles = local_app_data
                    .map(|root| relative.iter().fold(root.to_path_buf(), |path, part| path.join(part)))
                    .and_then(|dir| std::fs::read_to_string(dir.join("Local State")).ok())
                    .map(|content| parse_chromium_profiles(&content))
                    .unwrap_or_default();
                if profiles.is_empty() {
                    targets.push(plain);
                }
                for (directory, name) in profiles {
                    targets.push(BrowserTarget {
                        id: format!("{}|{}", browser.key, directory),
                        label: format!("{} · {}", browser.name, name),
                        executable: browser.executable.clone(),
                        args: vec![format!("--profile-directory={directory}")],
                    });
                }
            }
            Family::Firefox => {
                let profiles = roaming_app_data
                    .and_then(|root| {
                        std::fs::read_to_string(root.join("Mozilla").join("Firefox").join("profiles.ini")).ok()
                    })
                    .map(|content| parse_firefox_profiles(&content))
                    .unwrap_or_default();
                targets.push(BrowserTarget {
                    label: format!("{}（默认配置）", browser.name),
                    ..plain
                });
                for name in profiles {
                    targets.push(BrowserTarget {
                        id: format!("{}|{}", browser.key, name),
                        label: format!("{} · {}", browser.name, name),
                        executable: browser.executable.clone(),
                        args: vec!["-P".into(), name],
                    });
                }
            }
            Family::Other => targets.push(plain),
        }
    }
    targets
}

/// Profile directory and display name from Chromium's `Local State`.
pub fn parse_chromium_profiles(local_state: &str) -> Vec<(String, String)> {
    let Ok(data) = serde_json::from_str::<Value>(local_state) else {
        return Vec::new();
    };
    let Some(cache) = data
        .pointer("/profile/info_cache")
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut profiles: Vec<(String, String)> = cache
        .iter()
        .filter(|(directory, _)| is_safe_profile_name(directory))
        .map(|(directory, info)| {
            let name = info
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or(directory.as_str());
            (directory.clone(), name.trim().to_string())
        })
        .collect();
    profiles.sort_by(|left, right| {
        (left.0 != "Default")
            .cmp(&(right.0 != "Default"))
            .then_with(|| natural_key(&left.0).cmp(&natural_key(&right.0)))
    });
    profiles
}

/// Profile names from Firefox's `profiles.ini`.
pub fn parse_firefox_profiles(ini: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_profile = false;
    for line in ini.lines().map(str::trim) {
        if line.starts_with('[') {
            in_profile = line.starts_with("[Profile");
        } else if in_profile {
            if let Some(name) = line.strip_prefix("Name=") {
                let name = name.trim();
                if is_safe_profile_name(name) && !names.iter().any(|known| known == name) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

fn is_safe_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('-')
        && !name.chars().any(|character| character.is_control() || character == '"')
}

fn natural_key(directory: &str) -> (u64, String) {
    let number = directory
        .rsplit(' ')
        .next()
        .and_then(|part| part.parse().ok())
        .unwrap_or(u64::MAX);
    (number, directory.to_string())
}

/// Extracts the executable from a registry `shell\open\command` value.
pub fn parse_command_executable(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split('"').next().map(PathBuf::from);
    }
    let lower = command.to_ascii_lowercase();
    lower
        .find(".exe")
        .map(|end| PathBuf::from(&command[..end + 4]))
}

#[cfg(windows)]
fn installed_browsers() -> Vec<InstalledBrowser> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let roots = [
        (HKEY_CURRENT_USER, r"SOFTWARE\Clients\StartMenuInternet"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Clients\StartMenuInternet"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Clients\StartMenuInternet"),
    ];
    let mut browsers: Vec<InstalledBrowser> = Vec::new();
    for (hive, path) in roots {
        let Ok(clients) = RegKey::predef(hive).open_subkey(path) else {
            continue;
        };
        for key in clients.enum_keys().flatten() {
            if key.eq_ignore_ascii_case("IEXPLORE.EXE")
                || browsers.iter().any(|known| known.key.eq_ignore_ascii_case(&key))
            {
                continue;
            }
            let Ok(client) = clients.open_subkey(&key) else {
                continue;
            };
            let name: String = client
                .get_value("")
                .ok()
                .filter(|name: &String| !name.trim().is_empty())
                .unwrap_or_else(|| key.clone());
            let Some(executable) = client
                .open_subkey(r"shell\open\command")
                .and_then(|command| command.get_value::<String, _>(""))
                .ok()
                .and_then(|command| parse_command_executable(&command))
                .filter(|path| path.is_file())
            else {
                continue;
            };
            browsers.push(InstalledBrowser {
                key,
                name: name.trim().to_string(),
                executable,
            });
        }
    }
    browsers.sort_by_key(|browser| browser.name.to_lowercase());
    browsers
}

#[cfg(not(windows))]
fn installed_browsers() -> Vec<InstalledBrowser> {
    Vec::new()
}

/// Opens `url` in the given browser target. The URL is always built by the
/// backend from fixed GitHub addresses, never taken from the page.
pub fn launch(target: &BrowserTarget, url: &str) -> Result<(), String> {
    Command::new(&target.executable)
        .args(&target.args)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| format!("无法启动 {}", target.label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_commands() {
        assert_eq!(
            parse_command_executable(r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --flag"#),
            Some(PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"))
        );
        assert_eq!(
            parse_command_executable(r"C:\Program Files\Internet Explorer\iexplore.exe"),
            Some(PathBuf::from(r"C:\Program Files\Internet Explorer\iexplore.exe"))
        );
        assert_eq!(parse_command_executable("nothing here"), None);
    }

    #[test]
    fn parses_chromium_profiles_default_first() {
        let state = r#"{"profile":{"info_cache":{
            "Profile 10":{"name":"Work"},"Profile 2":{"name":"Side"},
            "Default":{"name":"Personal"},"Profile 3":{"name":""}}}}"#;
        assert_eq!(
            parse_chromium_profiles(state),
            vec![
                ("Default".into(), "Personal".into()),
                ("Profile 2".into(), "Side".into()),
                ("Profile 3".into(), "Profile 3".into()),
                ("Profile 10".into(), "Work".into()),
            ]
        );
        assert!(parse_chromium_profiles("{}").is_empty());
        assert!(parse_chromium_profiles("broken").is_empty());
    }

    #[test]
    fn parses_firefox_profiles_and_skips_unsafe_names() {
        let ini = "[Install1]\nDefault=x\n[Profile1]\nName=default-release\nPath=a\n\
                   [Profile0]\nName=work\n[Profile2]\nName=-new-window\n[General]\nName=nope\n";
        assert_eq!(parse_firefox_profiles(ini), vec!["default-release", "work"]);
    }

    #[test]
    fn expands_profiles_per_browser_family() {
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("local");
        let roaming = dir.path().join("roaming");
        let chrome_data = local.join("Google").join("Chrome").join("User Data");
        std::fs::create_dir_all(&chrome_data).unwrap();
        std::fs::write(
            chrome_data.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Personal"},"Profile 1":{"name":"Work"}}}}"#,
        )
        .unwrap();
        let firefox_data = roaming.join("Mozilla").join("Firefox");
        std::fs::create_dir_all(&firefox_data).unwrap();
        std::fs::write(firefox_data.join("profiles.ini"), "[Profile0]\nName=dev\n").unwrap();

        let installed = vec![
            InstalledBrowser {
                key: "Google Chrome".into(),
                name: "Google Chrome".into(),
                executable: PathBuf::from(r"C:\Chrome\chrome.exe"),
            },
            InstalledBrowser {
                key: "Firefox-1".into(),
                name: "Mozilla Firefox".into(),
                executable: PathBuf::from(r"C:\Firefox\firefox.exe"),
            },
            InstalledBrowser {
                key: "Microsoft Edge".into(),
                name: "Microsoft Edge".into(),
                executable: PathBuf::from(r"C:\Edge\msedge.exe"),
            },
        ];
        let targets = expand_targets(&installed, Some(&local), Some(&roaming));
        let summary: Vec<(&str, &str, Vec<&str>)> = targets
            .iter()
            .map(|target| {
                (
                    target.id.as_str(),
                    target.label.as_str(),
                    target.args.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Google Chrome|Default", "Google Chrome · Personal", vec!["--profile-directory=Default"]),
                ("Google Chrome|Profile 1", "Google Chrome · Work", vec!["--profile-directory=Profile 1"]),
                ("Firefox-1|", "Mozilla Firefox（默认配置）", vec![]),
                ("Firefox-1|dev", "Mozilla Firefox · dev", vec!["-P", "dev"]),
                ("Microsoft Edge|", "Microsoft Edge", vec![]),
            ]
        );
    }
}
