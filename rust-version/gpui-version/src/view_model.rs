//! Presentation helpers shared by every GPUI widget. Pure functions only, so
//! they can be unit-tested without a window.

use ghdeck_core::AccountView;

/// Chinese label for the account state reported by gh.
pub fn state_label(state: ghdeck_core::AuthState) -> &'static str {
    use ghdeck_core::AuthState::*;
    match state {
        Authenticated => "已认证",
        NeedsLogin => "需重新认证",
        Timeout => "状态检查超时",
        Unknown => "认证状态未知",
    }
}

pub fn display_name(account: &AccountView) -> &str {
    account
        .github_login
        .as_deref()
        .unwrap_or(account.login.as_str())
}

/// Two-letter avatar text derived from the displayed name.
pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() > 1 {
        let mut text = String::new();
        for part in parts.iter().take(2) {
            if let Some(character) = part.chars().next() {
                text.extend(character.to_uppercase());
            }
        }
        return text;
    }
    name.chars().take(2).collect::<String>().to_uppercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeRisk {
    High,
    Elevated,
    Normal,
}

pub struct ScopeInfo {
    pub description: &'static str,
    pub risk: ScopeRisk,
}

/// OAuth scope explanations. Unknown scopes are shown without risk markup.
pub fn scope_info(scope: &str) -> ScopeInfo {
    let (description, risk) = match scope {
        "repo" => ("读写公开和私有仓库", ScopeRisk::Elevated),
        "public_repo" => ("读写公开仓库", ScopeRisk::Elevated),
        "repo:status" => ("读写提交状态", ScopeRisk::Normal),
        "repo_deployment" => ("读写部署状态", ScopeRisk::Normal),
        "repo:invite" => ("接受或拒绝协作邀请", ScopeRisk::Normal),
        "security_events" => ("读写安全事件", ScopeRisk::Elevated),
        "workflow" => ("修改 Actions 工作流文件", ScopeRisk::Elevated),
        "delete_repo" => ("删除仓库", ScopeRisk::High),
        "gist" => ("创建和修改 Gist", ScopeRisk::Normal),
        "notifications" => ("读取通知", ScopeRisk::Normal),
        "read:org" => ("读取组织与团队成员信息", ScopeRisk::Normal),
        "write:org" => ("管理组织与团队成员", ScopeRisk::Elevated),
        "admin:org" => ("完全管理组织", ScopeRisk::High),
        "read:public_key" => ("读取 SSH 公钥", ScopeRisk::Normal),
        "write:public_key" => ("添加 SSH 公钥", ScopeRisk::Elevated),
        "admin:public_key" => ("完全管理 SSH 公钥", ScopeRisk::High),
        "admin:repo_hook" => ("完全管理仓库 Webhook", ScopeRisk::High),
        "write:repo_hook" => ("修改仓库 Webhook", ScopeRisk::Elevated),
        "read:repo_hook" => ("读取仓库 Webhook", ScopeRisk::Normal),
        "admin:org_hook" => ("完全管理组织 Webhook", ScopeRisk::High),
        "read:gpg_key" => ("读取 GPG 密钥", ScopeRisk::Normal),
        "write:gpg_key" => ("添加 GPG 密钥", ScopeRisk::Elevated),
        "admin:gpg_key" => ("完全管理 GPG 密钥", ScopeRisk::High),
        "admin:ssh_signing_key" => ("完全管理 SSH 签名密钥", ScopeRisk::High),
        "user" => ("读写个人资料", ScopeRisk::Elevated),
        "read:user" => ("读取个人资料", ScopeRisk::Normal),
        "user:email" => ("读取邮箱地址", ScopeRisk::Normal),
        "user:follow" => ("关注或取消关注用户", ScopeRisk::Normal),
        "project" => ("读写 Projects", ScopeRisk::Normal),
        "read:project" => ("读取 Projects", ScopeRisk::Normal),
        "read:packages" => ("下载 Packages", ScopeRisk::Normal),
        "write:packages" => ("发布 Packages", ScopeRisk::Elevated),
        "delete:packages" => ("删除 Packages", ScopeRisk::High),
        "codespace" => ("管理 Codespaces", ScopeRisk::Elevated),
        "copilot" => ("管理 Copilot 设置", ScopeRisk::Normal),
        "audit_log" | "read:audit_log" => ("读取审计日志", ScopeRisk::Normal),
        "admin:enterprise" => ("完全管理企业", ScopeRisk::High),
        "site_admin" => ("GitHub Enterprise Server 站点管理员", ScopeRisk::High),
        _ => ("未收录的权限", ScopeRisk::Normal),
    };
    ScopeInfo { description, risk }
}

pub fn high_risk_scopes(scopes: &[String]) -> Vec<&str> {
    scopes
        .iter()
        .filter(|scope| scope_info(scope).risk == ScopeRisk::High)
        .map(String::as_str)
        .collect()
}

/// High risk first, then elevated, then the rest, each group alphabetical.
pub fn sorted_scopes(scopes: &[String]) -> Vec<&String> {
    let weight = |scope: &String| match scope_info(scope).risk {
        ScopeRisk::High => 0,
        ScopeRisk::Elevated => 1,
        ScopeRisk::Normal => 2,
    };
    let mut sorted: Vec<&String> = scopes.iter().collect();
    sorted.sort_by(|left, right| weight(left).cmp(&weight(right)).then_with(|| left.cmp(right)));
    sorted
}

/// A browser choice offered for binding and for the login dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserChoice {
    pub id: String,
    pub label: String,
}

/// Dialog currently covering the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    None,
    Settings {
        path: String,
        status: String,
        status_kind: StatusKind,
        busy: bool,
    },
    Login {
        browsers: Vec<BrowserChoice>,
        selected: Option<String>,
    },
    /// Confirmation for a destructive action.
    Confirm {
        title: String,
        body: String,
        action: ConfirmAction,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Idle,
    Busy,
    #[allow(dead_code)]
    Success,
    #[allow(dead_code)]
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Logout { host: String, login: String },
    SyncRename { host: String, login: String },
}

impl Dialog {
    pub fn is_none(&self) -> bool {
        matches!(self, Dialog::None)
    }
}

/// Without a hover system of its own, the GPUI shell keeps selection and
/// expansion state in one place; this is the pure part of it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    /// `host|login` of the card whose details are expanded.
    pub expanded: Option<String>,
}

impl Selection {
    /// Keeps the expansion only while the account still exists.
    pub fn reconcile(&mut self, accounts: &[AccountView]) {
        if let Some(key) = &self.expanded {
            let key = key.clone();
            if !accounts.iter().any(|account| Self::key(account) == key) {
                self.expanded = None;
            }
        }
    }

    pub fn key(account: &AccountView) -> String {
        ghdeck_core::account_key(&account.host, &account.login)
    }

    pub fn is_expanded(&self, account: &AccountView) -> bool {
        self.expanded.as_deref() == Some(Self::key(account).as_str())
    }

    /// Clicking the expanded card collapses it.
    pub fn toggle(&mut self, account: &AccountView) {
        let key = Self::key(account);
        self.expanded = if self.expanded.as_deref() == Some(key.as_str()) {
            None
        } else {
            Some(key)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghdeck_core::{AccountView, AuthState};

    fn account(host: &str, login: &str, github: Option<&str>) -> AccountView {
        AccountView {
            host: host.into(),
            login: login.into(),
            github_login: github.map(str::to_string),
            renamed: github.is_some_and(|github| github != login),
            active: false,
            state: AuthState::Authenticated,
            scopes: vec![],
            browser: None,
            browser_available: false,
        }
    }

    #[test]
    fn initials_take_two_letters_from_one_word() {
        assert_eq!(initials("alice"), "AL");
        assert_eq!(initials("worked-bot"), "WB");
        assert_eq!(initials("a-b-c"), "AB");
        assert_eq!(initials("x"), "X");
    }

    #[test]
    fn scopes_sort_by_risk_then_name() {
        let scopes = vec![
            "gist".to_string(),
            "delete_repo".to_string(),
            "repo".to_string(),
            "admin:org".to_string(),
        ];
        assert_eq!(
            sorted_scopes(&scopes),
            vec!["admin:org", "delete_repo", "repo", "gist"]
        );
        assert_eq!(high_risk_scopes(&scopes), vec!["delete_repo", "admin:org"]);
    }

    #[test]
    fn display_name_prefers_the_github_name() {
        let renamed = account("github.com", "old", Some("new"));
        assert_eq!(display_name(&renamed), "new");
        let unknown = account("github.com", "old", None);
        assert_eq!(display_name(&unknown), "old");
    }

    #[test]
    fn selection_toggles_and_drops_vanished_accounts() {
        let mut selection = Selection::default();
        let alice = account("github.com", "alice", None);
        assert!(!selection.is_expanded(&alice));
        selection.toggle(&alice);
        assert!(selection.is_expanded(&alice));
        selection.toggle(&alice);
        assert!(!selection.is_expanded(&alice));

        selection.toggle(&alice);
        selection.reconcile(std::slice::from_ref(&alice));
        assert!(selection.is_expanded(&alice));
        selection.reconcile(&[]);
        assert_eq!(selection.expanded, None);
    }

    /// Binding a browser, re-authenticating or syncing a rename happens inside
    /// an expanded card. Those actions must leave the card open: collapsing it
    /// under the user's cursor would interrupt whatever they were doing.
    #[test]
    fn nested_card_actions_keep_the_card_expanded() {
        let mut selection = Selection::default();
        let alice = account("github.com", "alice", None);
        selection.toggle(&alice);

        // Each action re-renders with the same account list, as the polling
        // redraw does after the controller publishes a new snapshot.
        for _ in 0..3 {
            selection.reconcile(std::slice::from_ref(&alice));
            assert!(
                selection.is_expanded(&alice),
                "a nested action must not collapse the card"
            );
        }
    }
}
