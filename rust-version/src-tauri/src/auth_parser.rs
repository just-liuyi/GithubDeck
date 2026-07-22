use crate::models::{Account, AuthState};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthStatusError {
    #[error("GitHub CLI 返回了无法解析的状态")]
    InvalidJson,
    #[error("GitHub CLI 状态中缺少 hosts")]
    MissingHosts,
}

pub fn parse_auth_status(payload: &[u8]) -> Result<Vec<Account>, AuthStatusError> {
    let data: Value = serde_json::from_slice(payload).map_err(|_| AuthStatusError::InvalidJson)?;
    let hosts = data
        .get("hosts")
        .and_then(|value| value.as_object())
        .ok_or(AuthStatusError::MissingHosts)?;

    let mut accounts = Vec::new();
    for (host, entries) in hosts {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        for entry in entries {
            let Some(entry) = entry.as_object() else {
                continue;
            };
            let Some(login) = entry.get("login").and_then(|value| value.as_str()) else {
                continue;
            };
            if login.is_empty() {
                continue;
            }
            let active = entry
                .get("active")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let state = match entry.get("state").and_then(|value| value.as_str()) {
                Some("success") => AuthState::Authenticated,
                Some("failure") => AuthState::NeedsLogin,
                _ => AuthState::Unknown,
            };
            accounts.push(Account {
                host: host.clone(),
                login: login.to_string(),
                active,
                state,
            });
        }
    }

    accounts.sort_by(|left, right| {
        left.host
            .cmp(&right.host)
            .then_with(|| right.active.cmp(&left.active))
            .then_with(|| left.login.to_ascii_lowercase().cmp(&right.login.to_ascii_lowercase()))
    });
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accounts_and_active_state() {
        let payload = br#"{"hosts":{"github.com":[{"login":"alice","active":true,"state":"success"},{"login":"bob","active":false,"state":"failure"}]}}"#;
        let accounts = parse_auth_status(payload).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].login, "alice");
        assert!(accounts[0].active);
        assert_eq!(accounts[0].state, AuthState::Authenticated);
        assert_eq!(accounts[1].login, "bob");
        assert_eq!(accounts[1].state, AuthState::NeedsLogin);
    }

    #[test]
    fn parses_empty_hosts() {
        let accounts = parse_auth_status(br#"{"hosts":{}}"#).unwrap();
        assert!(accounts.is_empty());
    }

    #[test]
    fn rejects_invalid_status() {
        assert_eq!(
            parse_auth_status(b"not-json").unwrap_err(),
            AuthStatusError::InvalidJson
        );
        assert_eq!(
            parse_auth_status(br#"{"other":{}}"#).unwrap_err(),
            AuthStatusError::MissingHosts
        );
    }

    #[test]
    fn unknown_state_is_default_when_not_whitelisted() {
        let payload = br#"{"hosts":{"github.com":[{"login":"alice","state":"weird"}]}}"#;
        let accounts = parse_auth_status(payload).unwrap();
        assert_eq!(accounts[0].state, AuthState::Unknown);
    }
}
