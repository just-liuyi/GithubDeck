use serde::{Deserialize, Serialize};

/// Authentication state reported by `gh auth status --json hosts`.
///
/// gh emits `success`, `error`, `timeout` and `refresh_expired`; older builds
/// used `failure`. Anything else is kept as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AuthState {
    #[serde(rename = "success")]
    Authenticated,
    #[serde(rename = "failure")]
    NeedsLogin,
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "unknown")]
    #[default]
    Unknown,
}

/// One account as GitHub CLI stores it. `login` is the name gh saved at login
/// time; it does not follow renames on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub host: String,
    pub login: String,
    pub active: bool,
    pub state: AuthState,
    pub scopes: Vec<String>,
}

impl Account {
    pub fn key(&self) -> String {
        account_key(&self.host, &self.login)
    }
}

pub fn account_key(host: &str, login: &str) -> String {
    format!("{host}|{login}")
}
