//! Local facts GithubDeck keeps about each gh account: the GitHub user ID and
//! current login (to notice renames) and the browser the account lives in.
//! Stored next to settings.json; contains no credentials.

use crate::settings::{app_config_dir, write_file};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

const META_FILE_NAME: &str = "accounts.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBinding {
    pub id: String,
    /// Label at binding time, shown when the browser is no longer found.
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMeta {
    /// Only ever learned from `gh api user` run with this account's own token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<u64>,
    /// The login GitHub reported most recently for `user_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<BrowserBinding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetaFile {
    #[serde(default)]
    accounts: BTreeMap<String, AccountMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_login_browser: Option<BrowserBinding>,
}

pub struct MetaStore {
    file: Option<PathBuf>,
    data: MetaFile,
}

impl MetaStore {
    pub fn load() -> Self {
        Self::load_from(app_config_dir().join(META_FILE_NAME))
    }

    pub fn load_from(file: PathBuf) -> Self {
        let data = std::fs::read_to_string(&file)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default();
        Self {
            file: Some(file),
            data,
        }
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            file: None,
            data: MetaFile::default(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&AccountMeta> {
        self.data.accounts.get(key)
    }

    pub fn entry(&mut self, key: &str) -> &mut AccountMeta {
        self.data.accounts.entry(key.to_string()).or_default()
    }

    /// Moves what is known about `from` to `to` after gh re-keyed an account.
    /// Values already recorded for `to` win.
    pub fn migrate(&mut self, from: &str, to: &str) {
        let Some(old) = self.data.accounts.remove(from) else {
            return;
        };
        let new = self.entry(to);
        new.user_id = new.user_id.or(old.user_id);
        new.github_login = new.github_login.take().or(old.github_login);
        new.browser = new.browser.take().or(old.browser);
    }

    pub fn last_login_browser(&self) -> Option<&BrowserBinding> {
        self.data.last_login_browser.as_ref()
    }

    pub fn set_last_login_browser(&mut self, binding: Option<BrowserBinding>) {
        self.data.last_login_browser = binding;
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.data.accounts.retain(|_, meta| meta != &AccountMeta::default());
        let Some(file) = &self.file else {
            return Ok(());
        };
        let content =
            serde_json::to_string_pretty(&self.data).map_err(|_| "无法保存账号信息".to_string())?;
        write_file(file, &content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(id: &str) -> BrowserBinding {
        BrowserBinding {
            id: id.into(),
            label: format!("label {id}"),
        }
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("accounts.json");
        let mut store = MetaStore::load_from(file.clone());
        store.entry("github.com|alice").user_id = Some(7);
        store.entry("github.com|alice").browser = Some(binding("chrome|Default"));
        store.set_last_login_browser(Some(binding("edge|")));
        store.entry("github.com|empty");
        store.save().unwrap();

        let reloaded = MetaStore::load_from(file.clone());
        assert_eq!(reloaded.get("github.com|alice").unwrap().user_id, Some(7));
        assert!(reloaded.get("github.com|empty").is_none());
        assert_eq!(reloaded.last_login_browser().unwrap().id, "edge|");
        let text = std::fs::read_to_string(file).unwrap();
        assert!(!text.to_lowercase().contains("token"));
    }

    #[test]
    fn tolerates_corrupt_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("accounts.json");
        std::fs::write(&file, "{not json").unwrap();
        assert!(MetaStore::load_from(file).get("x").is_none());
    }

    #[test]
    fn migrate_keeps_existing_values_of_the_target() {
        let mut store = MetaStore::in_memory();
        store.entry("h|old").user_id = Some(1);
        store.entry("h|old").browser = Some(binding("old-browser"));
        store.entry("h|new").github_login = Some("new".into());
        store.migrate("h|old", "h|new");
        assert!(store.get("h|old").is_none());
        let new = store.get("h|new").unwrap();
        assert_eq!(new.user_id, Some(1));
        assert_eq!(new.github_login.as_deref(), Some("new"));
        assert_eq!(new.browser.as_ref().unwrap().id, "old-browser");
    }
}
