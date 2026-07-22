use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthState {
    #[serde(rename = "success")]
    Authenticated,
    #[serde(rename = "failure")]
    NeedsLogin,
    #[serde(rename = "unknown")]
    Unknown,
}

impl Default for AuthState {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub host: String,
    pub login: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub state: AuthState,
}

impl Account {
    pub fn new(host: impl Into<String>, login: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            login: login.into(),
            active: false,
            state: AuthState::Unknown,
        }
    }

    pub fn key(&self) -> String {
        format!("{}|{}", self.host, self.login)
    }
}
