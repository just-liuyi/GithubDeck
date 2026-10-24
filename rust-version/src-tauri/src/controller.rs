//! Account operations and UI state.
//!
//! Each user action runs as one task that awaits its gh invocations in order,
//! so there is no event round trip and no raw gh output ever reaches the page:
//! the page only receives [`AppStateSnapshot`] values through the state sink.

use crate::account_meta::{BrowserBinding, MetaStore};
use crate::auth_parser::{parse_auth_status, parse_identity};
use crate::browsers::BrowserTarget;
use crate::gh_process::{
    GhRunner, RunOutput, RunRequest, RunStatus, API_TIMEOUT, LOGIN_TIMEOUT, LOGOUT_TIMEOUT,
    STATUS_TIMEOUT, SWITCH_TIMEOUT,
};
use crate::models::{account_key, Account, AuthState};
use crate::settings::{validate_executable, GhExecutableSettings, DEFAULT_GH_EXECUTABLE};
use serde::Serialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

const BUSY: &str = "当前有操作正在进行";
const GH_MISSING: &str = "未找到 GitHub CLI，请在设置中指定有效的 gh.exe";
const STALE: &str = "账号列表已变化，请刷新后重试";
const DEFAULT_BROWSER_LABEL: &str = "系统默认浏览器";

/// Opens fixed GitHub URLs in a chosen browser and lists installed browsers.
pub trait BrowserPlatform: Send + Sync + 'static {
    fn detect(&self) -> Vec<BrowserTarget>;
    /// `None` opens the system default browser.
    fn open(&self, target: Option<&BrowserTarget>, url: &str) -> Result<(), String>;
}

pub type StateSink = Arc<dyn Fn(AppStateSnapshot) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiMessage {
    pub text: String,
    pub is_error: bool,
}

impl UiMessage {
    fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub host: String,
    /// The name gh stored the account under.
    pub login: String,
    /// The name GitHub reports now, when known.
    pub github_login: Option<String>,
    pub renamed: bool,
    pub active: bool,
    pub state: AuthState,
    pub scopes: Vec<String>,
    pub browser: Option<BrowserOption>,
    pub browser_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginView {
    pub purpose: &'static str,
    pub target_login: Option<String>,
    pub code: String,
    pub url: String,
    pub browser_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    pub accounts: Vec<AccountView>,
    pub busy: bool,
    pub login: Option<LoginView>,
    pub message: UiMessage,
    pub gh_path: String,
    pub browsers: Vec<BrowserOption>,
    pub last_login_browser: Option<String>,
}

#[derive(Debug, Clone)]
enum LoginPurpose {
    New,
    Reauth {
        login: String,
    },
    RenameSync {
        old_login: String,
        user_id: u64,
    },
}

impl LoginPurpose {
    fn kind(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Reauth { .. } => "reauth",
            Self::RenameSync { .. } => "rename",
        }
    }

    fn target(&self) -> Option<String> {
        match self {
            Self::New => None,
            Self::Reauth { login } => Some(login.clone()),
            Self::RenameSync { old_login, .. } => Some(old_login.clone()),
        }
    }
}

struct LoginSession {
    host: String,
    purpose: LoginPurpose,
    cancel: CancellationToken,
    code: String,
    browser: Option<BrowserTarget>,
}

struct Inner {
    settings: GhExecutableSettings,
    meta: MetaStore,
    browsers: Vec<BrowserTarget>,
    accounts: Vec<Account>,
    busy: bool,
    login: Option<LoginSession>,
    message: UiMessage,
    task: Option<JoinHandle<()>>,
}

impl Inner {
    fn find_account(&self, host: &str, login: &str) -> Option<Account> {
        self.accounts
            .iter()
            .find(|account| account.host == host && account.login == login)
            .cloned()
    }

    fn find_browser(&self, id: &str) -> Option<BrowserTarget> {
        self.browsers.iter().find(|target| target.id == id).cloned()
    }

    fn bound_browser(&self, key: &str) -> Option<BrowserTarget> {
        self.meta
            .get(key)
            .and_then(|meta| meta.browser.as_ref())
            .and_then(|binding| self.find_browser(&binding.id))
    }

    fn github_login(&self, account: &Account) -> Option<String> {
        self.meta
            .get(&account.key())
            .and_then(|meta| meta.github_login.clone())
    }

    fn user_id(&self, account: &Account) -> Option<u64> {
        self.meta.get(&account.key()).and_then(|meta| meta.user_id)
    }

    fn is_renamed(&self, account: &Account) -> bool {
        self.github_login(account)
            .is_some_and(|github_login| github_login != account.login)
    }

    fn display_name(&self, account: &Account) -> String {
        self.github_login(account)
            .unwrap_or_else(|| account.login.clone())
    }

    fn snapshot(&self) -> AppStateSnapshot {
        let accounts = self
            .accounts
            .iter()
            .map(|account| {
                let binding = self
                    .meta
                    .get(&account.key())
                    .and_then(|meta| meta.browser.as_ref());
                let current = binding.and_then(|binding| self.find_browser(&binding.id));
                AccountView {
                    host: account.host.clone(),
                    login: account.login.clone(),
                    github_login: self.github_login(account),
                    renamed: self.is_renamed(account),
                    active: account.active,
                    state: account.state,
                    scopes: account.scopes.clone(),
                    browser: binding.map(|binding| BrowserOption {
                        id: binding.id.clone(),
                        label: current
                            .as_ref()
                            .map_or_else(|| binding.label.clone(), |target| target.label.clone()),
                    }),
                    browser_available: current.is_some(),
                }
            })
            .collect();
        AppStateSnapshot {
            accounts,
            busy: self.busy,
            login: self.login.as_ref().map(|session| LoginView {
                purpose: session.purpose.kind(),
                target_login: session.purpose.target(),
                code: session.code.clone(),
                url: device_url(&session.host),
                browser_label: browser_label(session.browser.as_ref()),
            }),
            message: self.message.clone(),
            gh_path: self.settings.path_string(),
            browsers: self
                .browsers
                .iter()
                .map(|target| BrowserOption {
                    id: target.id.clone(),
                    label: target.label.clone(),
                })
                .collect(),
            last_login_browser: self
                .meta
                .last_login_browser()
                .filter(|binding| self.find_browser(&binding.id).is_some())
                .map(|binding| binding.id.clone()),
        }
    }
}

pub struct AuthController<R: GhRunner> {
    inner: Arc<Mutex<Inner>>,
    runner: Arc<R>,
    platform: Arc<dyn BrowserPlatform>,
    sink: StateSink,
}

impl<R: GhRunner> Clone for AuthController<R> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            runner: self.runner.clone(),
            platform: self.platform.clone(),
            sink: self.sink.clone(),
        }
    }
}

impl<R: GhRunner> AuthController<R> {
    pub fn new(
        runner: R,
        platform: Arc<dyn BrowserPlatform>,
        settings: GhExecutableSettings,
        meta: MetaStore,
        sink: StateSink,
    ) -> Self {
        let browsers = platform.detect();
        Self {
            inner: Arc::new(Mutex::new(Inner {
                settings,
                meta,
                browsers,
                accounts: Vec::new(),
                busy: false,
                login: None,
                message: UiMessage::info("准备就绪"),
                task: None,
            })),
            runner: Arc::new(runner),
            platform,
            sink,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn snapshot(&self) -> AppStateSnapshot {
        self.lock().snapshot()
    }

    fn emit(&self) {
        let snapshot = self.snapshot();
        (self.sink)(snapshot);
    }

    fn set_message(&self, message: UiMessage) {
        self.lock().message = message;
        self.emit();
    }

    fn fail(&self, text: &str) -> Result<(), String> {
        self.set_message(UiMessage::error(text));
        Err(text.to_string())
    }

    /// Marks the controller busy and returns the gh executable to use.
    fn begin(&self, message: String) -> Result<PathBuf, String> {
        let executable = {
            let mut inner = self.lock();
            if inner.busy {
                return Err(BUSY.into());
            }
            if !inner.settings.executable_available() {
                drop(inner);
                self.fail(GH_MISSING)?;
                unreachable!();
            }
            inner.busy = true;
            inner.message = UiMessage::info(message);
            inner.settings.path()
        };
        self.emit();
        Ok(executable)
    }

    fn finish(&self, message: UiMessage) {
        {
            let mut inner = self.lock();
            inner.busy = false;
            inner.message = message;
        }
        self.emit();
    }

    /// Only accounts from the last refresh may be acted on.
    fn trusted_account(&self, host: &str, login: &str) -> Result<Account, String> {
        let account = {
            let inner = self.lock();
            if inner.busy {
                return Err(BUSY.into());
            }
            inner.find_account(host, login)
        };
        match account {
            Some(account) => Ok(account),
            None => {
                self.fail(STALE)?;
                unreachable!()
            }
        }
    }

    fn spawn(&self, future: impl Future<Output = ()> + Send + 'static) {
        let handle = tokio::spawn(future);
        self.lock().task = Some(handle);
    }

    fn run(&self, executable: &Path, args: &[&str], timeout: Duration) -> impl Future<Output = RunOutput> + Send + '_ {
        self.runner.run(RunRequest::new(executable, args, timeout))
    }

    // ---- refresh -------------------------------------------------------

    pub fn refresh(&self) -> Result<(), String> {
        let executable = self.begin("正在刷新账号…".into())?;
        let this = self.clone();
        self.spawn(async move {
            let message = match this.refresh_flow(&executable).await {
                Ok(()) => this.refreshed_message(None),
                Err(error) => UiMessage::error(error),
            };
            this.finish(message);
        });
        Ok(())
    }

    async fn refresh_flow(&self, executable: &Path) -> Result<(), String> {
        let output = self
            .run(executable, &["auth", "status", "--json", "hosts"], STATUS_TIMEOUT)
            .await;
        let parsed = match output.status {
            // With --json gh exits 0 even when accounts have problems.
            RunStatus::Success | RunStatus::Failed => {
                parse_auth_status(&output.stdout).map_err(|error| error.to_string())
            }
            RunStatus::TimedOut => Err("刷新账号超时，请检查网络后重试".into()),
            RunStatus::SpawnFailed => Err("无法启动 GitHub CLI".into()),
            RunStatus::Cancelled => Err("刷新已取消".into()),
        };
        let accounts = match parsed {
            Ok(accounts) => accounts,
            Err(error) => {
                self.lock().accounts.clear();
                return Err(error);
            }
        };

        {
            let mut inner = self.lock();
            inner.accounts = accounts.clone();
            if !accounts.is_empty() {
                inner.message = UiMessage::info("正在核对 GitHub 用户名…");
            }
        }
        self.emit();
        self.identify(executable, &accounts).await;
        let _ = self.lock().meta.save();
        Ok(())
    }

    /// Learns user IDs and current GitHub logins.
    ///
    /// gh keeps the login an account had when it logged in, so a rename on
    /// GitHub never shows up in `gh auth status`. The user ID never changes:
    /// it is recorded from `gh api user`, which runs with the active
    /// account's own token, and every account with a known ID is then looked
    /// up by ID to read its current login.
    async fn identify(&self, executable: &Path, accounts: &[Account]) {
        for active in accounts
            .iter()
            .filter(|account| account.active && account.state == AuthState::Authenticated)
        {
            let output = self
                .run(executable, &["api", "user", "--hostname", &active.host], API_TIMEOUT)
                .await;
            if output.status != RunStatus::Success {
                continue;
            }
            let Some(identity) = parse_identity(&output.stdout) else {
                continue;
            };
            let mut inner = self.lock();
            // If the answer carries the name of another stored account, the
            // active account may have been switched outside GithubDeck since
            // the status call. That cannot be told apart from both names of a
            // renamed account being stored, so learn nothing rather than pin
            // someone else's ID on this account.
            let answered_as_other = accounts.iter().any(|other| {
                other.host == active.host
                    && other.login != active.login
                    && other.login.eq_ignore_ascii_case(&identity.login)
            });
            if answered_as_other {
                continue;
            }
            let meta = inner.meta.entry(&active.key());
            meta.user_id = Some(identity.id);
            meta.github_login = Some(identity.login);
        }

        let lookups: Vec<(String, String, u64)> = {
            let inner = self.lock();
            accounts
                .iter()
                .filter(|account| !account.active)
                .filter(|account| {
                    accounts.iter().any(|active| {
                        active.host == account.host
                            && active.active
                            && active.state == AuthState::Authenticated
                    })
                })
                .filter_map(|account| {
                    inner
                        .user_id(account)
                        .map(|id| (account.key(), account.host.clone(), id))
                })
                .collect()
        };
        let mut lookups_running = JoinSet::new();
        for (key, host, id) in lookups {
            let runner = self.runner.clone();
            let executable = executable.to_path_buf();
            lookups_running.spawn(async move {
                let path = format!("user/{id}");
                let output = runner
                    .run(RunRequest::new(
                        executable,
                        &["api", path.as_str(), "--hostname", host.as_str()],
                        API_TIMEOUT,
                    ))
                    .await;
                (key, id, output)
            });
        }
        while let Some(joined) = lookups_running.join_next().await {
            let Ok((key, id, output)) = joined else {
                continue;
            };
            if output.status != RunStatus::Success {
                continue;
            }
            if let Some(identity) = parse_identity(&output.stdout).filter(|identity| identity.id == id) {
                self.lock().meta.entry(&key).github_login = Some(identity.login);
            }
        }
    }

    fn refreshed_message(&self, base: Option<String>) -> UiMessage {
        let inner = self.lock();
        let text = base.unwrap_or_else(|| {
            if inner.accounts.is_empty() {
                "尚未登录 GitHub 账号".into()
            } else {
                "账号列表已刷新".into()
            }
        });
        let count = |predicate: &dyn Fn(&Account) -> bool| {
            inner.accounts.iter().filter(|account| predicate(account)).count()
        };
        let renamed = count(&|account| inner.is_renamed(account));
        let needs_login = count(&|account| account.state == AuthState::NeedsLogin);
        let timed_out = count(&|account| account.state == AuthState::Timeout);
        let mut notes = Vec::new();
        if renamed > 0 {
            notes.push(format!("{renamed} 个账号已在 GitHub 改名"));
        }
        if needs_login > 0 {
            notes.push(format!("{needs_login} 个账号需要重新认证"));
        }
        if timed_out > 0 {
            notes.push(format!("{timed_out} 个账号状态检查超时"));
        }
        if notes.is_empty() {
            UiMessage::info(text)
        } else {
            UiMessage {
                text: format!("{text}；{}", notes.join("；")),
                is_error: needs_login + timed_out > 0,
            }
        }
    }

    async fn finish_with_refresh(&self, executable: &Path, success: String) {
        let message = match self.refresh_flow(executable).await {
            Ok(()) => self.refreshed_message(Some(success)),
            Err(error) => UiMessage::error(format!("{success}，但刷新失败：{error}")),
        };
        self.finish(message);
    }

    // ---- switch / logout -------------------------------------------------

    pub fn switch_account(&self, host: String, login: String) -> Result<(), String> {
        let account = self.trusted_account(&host, &login)?;
        let name = self.lock().display_name(&account);
        let executable = self.begin(format!("正在切换到 {name}…"))?;
        let this = self.clone();
        self.spawn(async move {
            let output = this
                .run(
                    &executable,
                    &["auth", "switch", "--hostname", &host, "--user", &login],
                    SWITCH_TIMEOUT,
                )
                .await;
            match failure_text(output.status, "切换账号") {
                Some(error) => this.finish(UiMessage::error(error)),
                None => {
                    this.finish_with_refresh(&executable, format!("已切换到 {name}"))
                        .await
                }
            }
        });
        Ok(())
    }

    pub fn logout(&self, host: String, login: String) -> Result<(), String> {
        let account = self.trusted_account(&host, &login)?;
        let name = self.lock().display_name(&account);
        let executable = self.begin(format!("正在登出 {name}…"))?;
        let this = self.clone();
        self.spawn(async move {
            let output = this
                .run(
                    &executable,
                    &["auth", "logout", "--hostname", &host, "--user", &login],
                    LOGOUT_TIMEOUT,
                )
                .await;
            match failure_text(output.status, "登出账号") {
                Some(error) => this.finish(UiMessage::error(error)),
                None => {
                    this.finish_with_refresh(&executable, format!("已登出 {name}"))
                        .await
                }
            }
        });
        Ok(())
    }

    // ---- login -------------------------------------------------------------

    /// Adds an account, authorizing in `browser_id` (system default if none).
    pub fn login_new(&self, browser_id: Option<String>) -> Result<(), String> {
        let browser = {
            let mut inner = self.lock();
            if inner.busy {
                return Err(BUSY.into());
            }
            let browser = match browser_id.filter(|id| !id.is_empty()) {
                None => None,
                Some(id) => Some(
                    inner
                        .find_browser(&id)
                        .ok_or_else(|| "所选浏览器已不可用，请重新扫描浏览器".to_string())?,
                ),
            };
            inner
                .meta
                .set_last_login_browser(browser.as_ref().map(binding_of));
            let _ = inner.meta.save();
            browser
        };
        self.start_login("github.com".into(), LoginPurpose::New, browser)
    }

    /// Logs in again for an account whose token stopped working.
    pub fn reauthenticate(&self, host: String, login: String) -> Result<(), String> {
        let account = self.trusted_account(&host, &login)?;
        let browser = self.lock().bound_browser(&account.key());
        self.start_login(host, LoginPurpose::Reauth { login }, browser)
    }

    /// Makes gh store a renamed account under its new name and drops the
    /// record kept under the old one.
    pub fn sync_rename(&self, host: String, login: String) -> Result<(), String> {
        let account = self.trusted_account(&host, &login)?;
        let (user_id, already_stored, browser) = {
            let inner = self.lock();
            let (Some(user_id), Some(new_login)) =
                (inner.user_id(&account), inner.github_login(&account))
            else {
                return Err("没有检测到该账号改名".into());
            };
            if new_login == login {
                return Err("没有检测到该账号改名".into());
            }
            let already_stored = inner
                .accounts
                .iter()
                .find(|other| {
                    other.host == host
                        && other.login == new_login
                        && inner.user_id(other) == Some(user_id)
                })
                .map(|other| other.login.clone());
            (user_id, already_stored, inner.bound_browser(&account.key()))
        };

        match already_stored {
            // The new name is already logged in; only the old record is left.
            Some(new_login) => {
                let executable = self.begin(format!("正在移除旧记录 {login}…"))?;
                let this = self.clone();
                self.spawn(async move {
                    this.remove_old_record(&executable, &host, &login, &new_login)
                        .await;
                });
                Ok(())
            }
            None => self.start_login(
                host,
                LoginPurpose::RenameSync {
                    old_login: login,
                    user_id,
                },
                browser,
            ),
        }
    }

    fn start_login(
        &self,
        host: String,
        purpose: LoginPurpose,
        browser: Option<BrowserTarget>,
    ) -> Result<(), String> {
        if !is_valid_host(&host) {
            return Err("不支持的 GitHub 主机".into());
        }
        let executable = self.begin("正在向 GitHub 申请一次性授权码…".into())?;
        let cancel = CancellationToken::new();
        self.lock().login = Some(LoginSession {
            host: host.clone(),
            purpose,
            cancel: cancel.clone(),
            code: String::new(),
            browser,
        });
        self.emit();

        let (code_sender, code_receiver) = oneshot::channel();
        let mut request = RunRequest::new(
            &executable,
            &["auth", "login", "--hostname", &host, "--git-protocol", "https", "--web"],
            LOGIN_TIMEOUT,
        );
        request.cancel = Some(cancel);
        request.device_code = Some(code_sender);

        let this = self.clone();
        self.spawn(async move {
            let run = this.runner.run(request);
            tokio::pin!(run);
            let mut code_receiver = Some(code_receiver);
            let output = loop {
                tokio::select! {
                    biased;
                    code = async { code_receiver.as_mut().expect("guarded").await },
                        if code_receiver.is_some() =>
                    {
                        code_receiver = None;
                        if let Ok(code) = code {
                            this.on_device_code(code);
                        }
                    }
                    output = &mut run => break output,
                }
            };
            if let Some(Ok(code)) = code_receiver.map(|mut receiver| receiver.try_recv()) {
                this.on_device_code(code);
            }
            this.after_login(&executable, output).await;
        });
        Ok(())
    }

    fn on_device_code(&self, code: String) {
        let (target, url) = {
            let mut inner = self.lock();
            let Some(session) = inner.login.as_mut() else {
                return;
            };
            if session.cancel.is_cancelled() {
                return;
            }
            session.code = code.clone();
            (session.browser.clone(), device_url(&session.host))
        };
        let label = browser_label(target.as_ref());
        let message = match self.platform.open(target.as_ref(), &url) {
            Ok(()) => UiMessage::info(format!("已在 {label} 打开授权页，请输入授权码 {code}")),
            Err(error) => UiMessage::error(format!("{error}，请复制网址手动打开")),
        };
        self.set_message(message);
    }

    pub fn reopen_authorization_page(&self) -> Result<(), String> {
        let (target, url) = {
            let inner = self.lock();
            match inner.login.as_ref() {
                Some(session) if !session.code.is_empty() => {
                    (session.browser.clone(), device_url(&session.host))
                }
                _ => return Err("当前没有等待中的授权".into()),
            }
        };
        self.platform.open(target.as_ref(), &url)
    }

    pub fn cancel_login(&self) -> Result<(), String> {
        {
            let mut inner = self.lock();
            let Some(session) = inner.login.as_ref() else {
                return Err("当前没有可取消的登录".into());
            };
            session.cancel.cancel();
            inner.message = UiMessage::info("正在取消登录…");
        }
        self.emit();
        Ok(())
    }

    async fn after_login(&self, executable: &Path, output: RunOutput) {
        let Some(session) = self.lock().login.take() else {
            self.finish(UiMessage::error("登录状态异常，请刷新后重试"));
            return;
        };
        let failure = match output.status {
            RunStatus::Success => None,
            RunStatus::Cancelled => Some(UiMessage::info("登录已取消")),
            RunStatus::TimedOut => Some(UiMessage::error("授权码已过期，请重新登录")),
            RunStatus::SpawnFailed => Some(UiMessage::error("无法启动 GitHub CLI")),
            RunStatus::Failed => Some(UiMessage::error(
                "登录失败，请确认已在浏览器中完成授权后重试",
            )),
        };
        if let Some(message) = failure {
            self.finish(message);
            return;
        }

        self.set_message(UiMessage::info("登录成功，正在刷新确认…"));
        if let Err(error) = self.refresh_flow(executable).await {
            self.finish(UiMessage::error(format!("登录成功，但刷新失败：{error}")));
            return;
        }

        let host = session.host;
        let (active, active_id, active_name) = {
            let inner = self.lock();
            let active = inner
                .accounts
                .iter()
                .find(|account| account.host == host && account.active)
                .cloned();
            let id = active.as_ref().and_then(|account| inner.user_id(account));
            let name = active.as_ref().map(|account| inner.display_name(account));
            (active, id, name.unwrap_or_default())
        };
        let Some(active) = active else {
            self.finish(self.refreshed_message(Some("登录完成".into())));
            return;
        };

        match session.purpose {
            LoginPurpose::New => {
                let mut text = format!("已登录 {active_name}");
                if let Some(target) = &session.browser {
                    let mut inner = self.lock();
                    inner.meta.entry(&active.key()).browser = Some(binding_of(target));
                    let _ = inner.meta.save();
                    text.push_str(&format!("，已绑定 {}", target.label));
                }
                self.finish(self.refreshed_message(Some(text)));
            }
            LoginPurpose::Reauth { login } => {
                let expected_id = self
                    .lock()
                    .meta
                    .get(&account_key(&host, &login))
                    .and_then(|meta| meta.user_id);
                if active.login == login {
                    self.finish(self.refreshed_message(Some(format!("{active_name} 重新认证成功"))));
                } else if expected_id.is_some() && expected_id == active_id {
                    self.finish(self.refreshed_message(Some(format!(
                        "重新认证成功；GitHub CLI 已以新名称 {} 保存该账号，可在旧记录 {login} 上点击“同步改名”移除旧记录",
                        active.login
                    ))));
                } else {
                    self.finish(UiMessage::error(format!(
                        "浏览器中授权的是 {active_name}，不是 {login}；{login} 仍需重新认证，请在正确的浏览器中重试"
                    )));
                }
            }
            LoginPurpose::RenameSync { old_login, user_id } => {
                if active_id != Some(user_id) {
                    self.finish(UiMessage::error(format!(
                        "浏览器中授权的是 {active_name}，与要同步的账号不是同一个，未移除旧记录 {old_login}"
                    )));
                } else if active.login == old_login {
                    self.finish(UiMessage::error(
                        "GitHub CLI 仍以旧名称保存该账号，未做改动",
                    ));
                } else {
                    self.remove_old_record(executable, &host, &old_login, &active.login)
                        .await;
                }
            }
        }
    }

    async fn remove_old_record(&self, executable: &Path, host: &str, old_login: &str, new_login: &str) {
        let output = self
            .run(
                executable,
                &["auth", "logout", "--hostname", host, "--user", old_login],
                LOGOUT_TIMEOUT,
            )
            .await;
        if let Some(error) = failure_text(output.status, "移除旧记录") {
            self.finish(UiMessage::error(error));
            return;
        }
        {
            let mut inner = self.lock();
            inner
                .meta
                .migrate(&account_key(host, old_login), &account_key(host, new_login));
            let _ = inner.meta.save();
        }
        self.finish_with_refresh(
            executable,
            format!("已同步改名：GitHub CLI 现在以 {new_login} 保存该账号"),
        )
        .await;
    }

    // ---- browsers ---------------------------------------------------------

    pub fn set_account_browser(
        &self,
        host: String,
        login: String,
        browser_id: Option<String>,
    ) -> Result<(), String> {
        {
            let mut inner = self.lock();
            let Some(account) = inner.find_account(&host, &login) else {
                return Err(STALE.into());
            };
            let binding = match browser_id.filter(|id| !id.is_empty()) {
                None => None,
                Some(id) => Some(
                    inner
                        .find_browser(&id)
                        .map(|target| binding_of(&target))
                        .ok_or_else(|| "所选浏览器已不可用，请重新扫描浏览器".to_string())?,
                ),
            };
            let name = inner.display_name(&account);
            let message = match &binding {
                Some(binding) => format!("{name} 已绑定到 {}", binding.label),
                None => format!("已取消 {name} 的浏览器绑定"),
            };
            inner.meta.entry(&account.key()).browser = binding;
            inner.meta.save()?;
            inner.message = UiMessage::info(message);
        }
        self.emit();
        Ok(())
    }

    pub fn open_account_in_browser(&self, host: String, login: String) -> Result<(), String> {
        let (target, name) = {
            let inner = self.lock();
            let Some(account) = inner.find_account(&host, &login) else {
                return Err(STALE.into());
            };
            let binding = inner
                .meta
                .get(&account.key())
                .and_then(|meta| meta.browser.clone());
            let target = binding.as_ref().and_then(|binding| inner.find_browser(&binding.id));
            if let (Some(binding), None) = (&binding, &target) {
                return Err(format!("绑定的浏览器 {} 已不可用，请重新绑定", binding.label));
            }
            (target, inner.display_name(&account))
        };
        if !is_valid_host(&host) {
            return Err("不支持的 GitHub 主机".into());
        }
        self.platform.open(target.as_ref(), &format!("https://{host}/"))?;
        self.set_message(UiMessage::info(format!(
            "已在 {} 打开 GitHub（{name}）",
            browser_label(target.as_ref())
        )));
        Ok(())
    }

    pub fn rescan_browsers(&self) {
        let browsers = self.platform.detect();
        {
            let mut inner = self.lock();
            inner.message = UiMessage::info(format!("找到 {} 个浏览器配置", browsers.len()));
            inner.browsers = browsers;
        }
        self.emit();
    }

    // ---- settings ---------------------------------------------------------

    pub fn gh_path(&self) -> String {
        self.lock().settings.path_string()
    }

    pub async fn save_gh_path(&self, path: String) -> Result<String, String> {
        if self.lock().busy {
            return Err(BUSY.into());
        }
        let result = validate_executable(&path).await;
        if !result.valid {
            return Err(result.message);
        }
        let saved = {
            let mut inner = self.lock();
            if inner.busy {
                return Err(BUSY.into());
            }
            inner.settings.set_path(path.trim())?;
            inner.message = UiMessage::info("GitHub CLI 路径已保存");
            inner.settings.path_string()
        };
        self.emit();
        Ok(saved)
    }

    pub async fn reset_gh_path(&self) -> Result<String, String> {
        if self.lock().busy {
            return Err(BUSY.into());
        }
        let result = validate_executable(DEFAULT_GH_EXECUTABLE).await;
        if !result.valid {
            return Err(format!("默认路径不可用（{}），设置未修改", result.message));
        }
        let path = {
            let mut inner = self.lock();
            if inner.busy {
                return Err(BUSY.into());
            }
            inner.settings.reset()?;
            inner.message = UiMessage::info("已恢复默认 GitHub CLI 路径");
            inner.settings.path_string()
        };
        self.emit();
        Ok(path)
    }

    // ---- lifecycle ----------------------------------------------------------

    /// Stops a pending login and waits briefly for the running task.
    pub async fn shutdown(&self) {
        let handle = {
            let mut inner = self.lock();
            if let Some(session) = &inner.login {
                session.cancel.cancel();
            }
            inner.task.take()
        };
        if let Some(handle) = handle {
            let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
        }
    }

    #[cfg(test)]
    async fn wait_idle(&self) {
        loop {
            let handle = self.lock().task.take();
            match handle {
                Some(handle) => handle.await.expect("task panicked"),
                None => break,
            }
        }
    }
}

fn failure_text(status: RunStatus, action: &str) -> Option<String> {
    match status {
        RunStatus::Success => None,
        RunStatus::TimedOut => Some(format!("{action}超时，请稍后重试")),
        RunStatus::SpawnFailed => Some("无法启动 GitHub CLI".into()),
        RunStatus::Cancelled => Some(format!("{action}已取消")),
        RunStatus::Failed => Some(format!("{action}失败，请检查 GitHub CLI 配置后重试")),
    }
}

fn binding_of(target: &BrowserTarget) -> BrowserBinding {
    BrowserBinding {
        id: target.id.clone(),
        label: target.label.clone(),
    }
}

fn browser_label(target: Option<&BrowserTarget>) -> String {
    target.map_or_else(|| DEFAULT_BROWSER_LABEL.to_string(), |target| target.label.clone())
}

fn device_url(host: &str) -> String {
    format!("https://{host}/login/device")
}

fn is_valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-'))
        && !host.starts_with(['.', '-'])
}

#[cfg(test)]
mod tests;
