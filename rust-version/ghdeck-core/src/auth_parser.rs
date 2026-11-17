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
        .and_then(Value::as_object)
        .ok_or(AuthStatusError::MissingHosts)?;

    let mut accounts = Vec::new();
    for (host, entries) in hosts {
        let Some(entries) = entries.as_array() else {
            continue;
        };
        for entry in entries.iter().filter_map(Value::as_object) {
            let Some(login) = entry.get("login").and_then(Value::as_str) else {
                continue;
            };
            if login.is_empty() {
                continue;
            }
            let active = entry.get("active").and_then(Value::as_bool).unwrap_or(false);
            let state = match entry.get("state").and_then(Value::as_str) {
                Some("success") => AuthState::Authenticated,
                Some("error" | "refresh_expired" | "failure") => AuthState::NeedsLogin,
                Some("timeout") => AuthState::Timeout,
                _ => AuthState::Unknown,
            };
            let scopes = entry
                .get("scopes")
                .and_then(Value::as_str)
                .map(parse_scopes)
                .unwrap_or_default();
            accounts.push(Account {
                host: host.clone(),
                login: login.to_string(),
                active,
                state,
                scopes,
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

fn parse_scopes(raw: &str) -> Vec<String> {
    let mut scopes: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(str::to_string)
        .collect();
    scopes.sort();
    scopes.dedup();
    scopes
}

/// Identity returned by `gh api user` / `gh api user/{id}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubIdentity {
    pub id: u64,
    pub login: String,
}

pub fn parse_identity(payload: &[u8]) -> Option<GithubIdentity> {
    let data: Value = serde_json::from_slice(payload).ok()?;
    let id = data.get("id")?.as_u64()?;
    let login = data.get("login")?.as_str()?;
    if !is_valid_login(login) {
        return None;
    }
    Some(GithubIdentity {
        id,
        login: login.to_string(),
    })
}

/// GitHub logins: alphanumerics and single hyphens, at most 39 characters.
/// Enterprise hosts may also use `_` and `.`, so accept those too.
pub fn is_valid_login(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= 64
        && login
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accounts_active_state_and_scopes() {
        let payload = br#"{"hosts":{"github.com":[
            {"login":"alice","active":true,"state":"success","scopes":"repo, gist, delete_repo"},
            {"login":"bob","active":false,"state":"error","error":"HTTP 401"}]}}"#;
        let accounts = parse_auth_status(payload).unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].login, "alice");
        assert!(accounts[0].active);
        assert_eq!(accounts[0].state, AuthState::Authenticated);
        assert_eq!(accounts[0].scopes, vec!["delete_repo", "gist", "repo"]);
        assert_eq!(accounts[1].login, "bob");
        assert_eq!(accounts[1].state, AuthState::NeedsLogin);
        assert!(accounts[1].scopes.is_empty());
    }

    #[test]
    fn maps_every_state_gh_reports() {
        let payload = br#"{"hosts":{"github.com":[
            {"login":"a","state":"success"},{"login":"b","state":"error"},
            {"login":"c","state":"refresh_expired"},{"login":"d","state":"timeout"},
            {"login":"e","state":"failure"},{"login":"f","state":"weird"}]}}"#;
        let states: Vec<AuthState> = parse_auth_status(payload)
            .unwrap()
            .into_iter()
            .map(|account| account.state)
            .collect();
        assert_eq!(
            states,
            vec![
                AuthState::Authenticated,
                AuthState::NeedsLogin,
                AuthState::NeedsLogin,
                AuthState::Timeout,
                AuthState::NeedsLogin,
                AuthState::Unknown
            ]
        );
    }

    #[test]
    fn parses_empty_hosts() {
        assert!(parse_auth_status(br#"{"hosts":{}}"#).unwrap().is_empty());
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
    fn parses_identity_and_rejects_bad_logins() {
        assert_eq!(
            parse_identity(br#"{"login":"new-name","id":42,"name":"x"}"#),
            Some(GithubIdentity {
                id: 42,
                login: "new-name".into()
            })
        );
        assert_eq!(parse_identity(br#"{"login":"a b","id":42}"#), None);
        assert_eq!(parse_identity(br#"{"login":"ok"}"#), None);
        assert_eq!(parse_identity(b"oops"), None);
    }
}
