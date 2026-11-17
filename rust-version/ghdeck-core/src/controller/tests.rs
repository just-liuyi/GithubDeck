use super::*;
use crate::gh_process::RunOutput;
use std::collections::BTreeMap;

/// One GitHub account as the fake gh sees it.
#[derive(Clone)]
struct FakeAccount {
    stored_login: String,
    id: u64,
    state: &'static str,
}

/// An in-memory GitHub + gh: stored accounts, the active one, and the names
/// accounts currently have on GitHub (to simulate renames).
#[derive(Default)]
struct World {
    accounts: Vec<FakeAccount>,
    active: Option<String>,
    github_names: BTreeMap<u64, String>,
    /// Account that the browser authorizes on the next `gh auth login`.
    next_login: Option<u64>,
    /// Login waits for cancellation instead of completing.
    login_hangs: bool,
    /// `gh api user` answers as this account (simulates an outside switch).
    api_user_override: Option<u64>,
    status_result: Option<RunStatus>,
    calls: Vec<Vec<String>>,
}

impl World {
    fn add(&mut self, login: &str, id: u64, state: &'static str) {
        self.accounts.push(FakeAccount {
            stored_login: login.into(),
            id,
            state,
        });
        self.github_names.entry(id).or_insert_with(|| login.into());
        if self.active.is_none() {
            self.active = Some(login.into());
        }
    }

    fn identity_json(&self, id: u64) -> Vec<u8> {
        format!(r#"{{"login":"{}","id":{id}}}"#, self.github_names[&id]).into_bytes()
    }

    fn status_json(&self) -> Vec<u8> {
        let entries: Vec<String> = self
            .accounts
            .iter()
            .map(|account| {
                format!(
                    r#"{{"login":"{}","active":{},"state":"{}","scopes":"repo, gist"}}"#,
                    account.stored_login,
                    self.active.as_deref() == Some(account.stored_login.as_str()),
                    account.state
                )
            })
            .collect();
        format!(r#"{{"hosts":{{"github.com":[{}]}}}}"#, entries.join(",")).into_bytes()
    }

    fn arg_after<'a>(args: &'a [String], flag: &str) -> &'a str {
        let index = args.iter().position(|arg| arg == flag).unwrap();
        &args[index + 1]
    }

    fn answer(&mut self, args: &[String]) -> RunOutput {
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["auth", "status", ..] => match self.status_result {
                Some(status) => RunOutput::new(status, Vec::new()),
                None => RunOutput::new(RunStatus::Success, self.status_json()),
            },
            ["api", "user", ..] => {
                let id = self.api_user_override.or_else(|| {
                    let active = self.active.as_deref()?;
                    self.accounts
                        .iter()
                        .find(|account| account.stored_login == active)
                        .map(|account| account.id)
                });
                match id {
                    Some(id) => RunOutput::new(RunStatus::Success, self.identity_json(id)),
                    None => RunOutput::new(RunStatus::Failed, Vec::new()),
                }
            }
            ["api", path, ..] if path.starts_with("user/") => {
                let id: u64 = path["user/".len()..].parse().unwrap();
                if self.github_names.contains_key(&id) {
                    RunOutput::new(RunStatus::Success, self.identity_json(id))
                } else {
                    RunOutput::new(RunStatus::Failed, Vec::new())
                }
            }
            ["auth", "switch", ..] => {
                self.active = Some(Self::arg_after(args, "--user").to_string());
                RunOutput::new(RunStatus::Success, Vec::new())
            }
            ["auth", "logout", ..] => {
                let login = Self::arg_after(args, "--user").to_string();
                self.accounts.retain(|account| account.stored_login != login);
                if self.active.as_deref() == Some(login.as_str()) {
                    self.active = self.accounts.first().map(|account| account.stored_login.clone());
                }
                RunOutput::new(RunStatus::Success, Vec::new())
            }
            ["auth", "login", ..] => {
                // gh stores the account under the name GitHub reports now.
                let id = self.next_login.expect("test must set next_login");
                let login = self.github_names[&id].clone();
                self.accounts.retain(|account| account.stored_login != login);
                self.accounts.push(FakeAccount {
                    stored_login: login.clone(),
                    id,
                    state: "success",
                });
                self.active = Some(login);
                RunOutput::new(RunStatus::Success, Vec::new())
            }
            other => panic!("unexpected gh call: {other:?}"),
        }
    }
}

#[derive(Clone, Default)]
struct FakeGh {
    world: Arc<Mutex<World>>,
}

impl GhRunner for FakeGh {
    fn run(&self, request: RunRequest) -> impl Future<Output = RunOutput> + Send {
        let world = self.world.clone();
        async move {
            let is_login = request.args.first().map(String::as_str) == Some("auth")
                && request.args.get(1).map(String::as_str) == Some("login");
            let hangs = {
                let mut world = world.lock().unwrap();
                world.calls.push(request.args.clone());
                is_login && world.login_hangs
            };
            if is_login {
                if let Some(sender) = request.device_code {
                    let _ = sender.send("ABCD-1234".into());
                }
                tokio::task::yield_now().await;
                if hangs {
                    request.cancel.expect("login is cancellable").cancelled().await;
                    return RunOutput::new(RunStatus::Cancelled, Vec::new());
                }
            }
            world.lock().unwrap().answer(&request.args)
        }
    }
}

#[derive(Default)]
struct FakePlatform {
    browsers: Vec<BrowserTarget>,
    opened: Mutex<Vec<(Option<String>, String)>>,
}

impl BrowserPlatform for FakePlatform {
    fn detect(&self) -> Vec<BrowserTarget> {
        self.browsers.clone()
    }

    fn open(&self, target: Option<&BrowserTarget>, url: &str) -> Result<(), String> {
        self.opened
            .lock()
            .unwrap()
            .push((target.map(|target| target.id.clone()), url.to_string()));
        Ok(())
    }
}

fn chrome() -> BrowserTarget {
    BrowserTarget {
        id: "Google Chrome|Default".into(),
        label: "Google Chrome · Personal".into(),
        executable: PathBuf::from(r"C:\chrome.exe"),
        args: vec!["--profile-directory=Default".into()],
    }
}

struct Harness {
    controller: AuthController<FakeGh>,
    gh: FakeGh,
    platform: Arc<FakePlatform>,
    _dir: tempfile::TempDir,
}

impl Harness {
    fn new(setup: impl FnOnce(&mut World)) -> Self {
        Self::with_meta(setup, MetaStore::in_memory())
    }

    fn with_meta(setup: impl FnOnce(&mut World), meta: MetaStore) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("gh.exe");
        std::fs::write(&executable, b"").unwrap();
        let mut settings = GhExecutableSettings::load_from(dir.path().join("settings.json"));
        settings.set_path(&executable).unwrap();

        let gh = FakeGh::default();
        setup(&mut gh.world.lock().unwrap());
        let platform = Arc::new(FakePlatform {
            browsers: vec![chrome()],
            ..Default::default()
        });
        let controller = AuthController::new(
            gh.clone(),
            platform.clone(),
            settings,
            meta,
            Arc::new(|_| {}),
        );
        Self {
            controller,
            gh,
            platform,
            _dir: dir,
        }
    }

    async fn refresh(&self) {
        self.controller.refresh().unwrap();
        self.controller.wait_idle().await;
    }

    fn account(&self, login: &str) -> AccountView {
        self.controller
            .snapshot()
            .accounts
            .into_iter()
            .find(|account| account.login == login)
            .unwrap_or_else(|| panic!("no account {login}"))
    }

    fn logins(&self) -> Vec<String> {
        self.controller
            .snapshot()
            .accounts
            .into_iter()
            .map(|account| account.login)
            .collect()
    }

    fn calls(&self) -> Vec<String> {
        self.gh
            .world
            .lock()
            .unwrap()
            .calls
            .iter()
            .map(|call| call.join(" "))
            .collect()
    }

    fn message(&self) -> UiMessage {
        self.controller.snapshot().message
    }
}

fn meta_with(entries: &[(&str, u64)]) -> MetaStore {
    let mut meta = MetaStore::in_memory();
    for (login, id) in entries {
        meta.entry(&account_key("github.com", login)).user_id = Some(*id);
    }
    meta
}

#[tokio::test]
async fn refresh_lists_accounts_and_learns_the_active_identity() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.add("bob", 2, "error");
    });
    harness.refresh().await;

    let alice = harness.account("alice");
    assert!(alice.active);
    assert_eq!(alice.github_login.as_deref(), Some("alice"));
    assert!(!alice.renamed);
    assert_eq!(alice.scopes, vec!["gist", "repo"]);
    let bob = harness.account("bob");
    assert_eq!(bob.state, AuthState::NeedsLogin);
    assert_eq!(bob.github_login, None, "bob's id was never learned");
    assert!(!harness.calls().iter().any(|call| call.starts_with("api user/")));
    let message = harness.message();
    assert!(message.text.contains("1 个账号需要重新认证"), "{message:?}");
    assert!(!harness.controller.snapshot().busy);
}

#[tokio::test]
async fn detects_a_rename_of_the_active_account() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.github_names.insert(1, "alice-new".into());
    });
    harness.refresh().await;
    let alice = harness.account("alice");
    assert_eq!(alice.github_login.as_deref(), Some("alice-new"));
    assert!(alice.renamed);
    assert!(harness.message().text.contains("1 个账号已在 GitHub 改名"));
}

#[tokio::test]
async fn detects_a_rename_of_an_inactive_account_by_its_known_id() {
    let harness = Harness::with_meta(
        |world| {
            world.add("alice", 1, "success");
            world.add("bob", 2, "success");
            world.github_names.insert(2, "bobby".into());
        },
        meta_with(&[("bob", 2)]),
    );
    harness.refresh().await;
    assert!(harness.calls().contains(&"api user/2 --hostname github.com".to_string()));
    let bob = harness.account("bob");
    assert_eq!(bob.github_login.as_deref(), Some("bobby"));
    assert!(bob.renamed);
}

#[tokio::test]
async fn ignores_an_identity_that_belongs_to_another_stored_account() {
    let harness = Harness::with_meta(
        |world| {
            world.add("alice", 1, "success");
            world.add("bob", 2, "success");
            world.api_user_override = Some(2);
        },
        meta_with(&[("bob", 2)]),
    );
    harness.refresh().await;
    let alice = harness.account("alice");
    assert_eq!(alice.github_login, None, "bob's identity must not be pinned on alice");
    assert!(!alice.renamed);
}

#[tokio::test]
async fn status_timeout_reports_a_timeout() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.status_result = Some(RunStatus::TimedOut);
    });
    harness.refresh().await;
    let message = harness.message();
    assert!(message.is_error);
    assert!(message.text.contains("超时"), "{message:?}");
    assert!(harness.logins().is_empty());
}

#[tokio::test]
async fn actions_on_accounts_not_in_the_last_refresh_are_refused() {
    let harness = Harness::new(|world| world.add("alice", 1, "success"));
    harness.refresh().await;
    let calls_before = harness.calls().len();
    assert_eq!(
        harness.controller.switch_account("github.com".into(), "mallory".into()),
        Err(STALE.to_string())
    );
    assert_eq!(
        harness.controller.logout("github.com".into(), "mallory".into()),
        Err(STALE.to_string())
    );
    assert_eq!(harness.calls().len(), calls_before);
}

#[tokio::test]
async fn switch_runs_gh_and_refreshes() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.add("bob", 2, "success");
    });
    harness.refresh().await;
    harness
        .controller
        .switch_account("github.com".into(), "bob".into())
        .unwrap();
    harness.controller.wait_idle().await;
    assert!(harness
        .calls()
        .contains(&"auth switch --hostname github.com --user bob".to_string()));
    assert!(harness.account("bob").active);
    // bob's identity is learned now that he is active.
    assert_eq!(harness.account("bob").github_login.as_deref(), Some("bob"));
    assert!(harness.message().text.starts_with("已切换到 bob"));
}

#[tokio::test]
async fn new_login_opens_the_chosen_browser_and_binds_it() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.github_names.insert(3, "carol".into());
        world.next_login = Some(3);
    });
    harness.refresh().await;
    harness
        .controller
        .login_new(Some(chrome().id))
        .unwrap();
    harness.controller.wait_idle().await;

    assert_eq!(
        harness.platform.opened.lock().unwrap().clone(),
        vec![(Some(chrome().id), "https://github.com/login/device".to_string())]
    );
    let carol = harness.account("carol");
    assert!(carol.active);
    assert_eq!(carol.browser.unwrap().id, chrome().id);
    assert!(carol.browser_available);
    let snapshot = harness.controller.snapshot();
    assert_eq!(snapshot.last_login_browser, Some(chrome().id));
    assert!(snapshot.login.is_none());
    assert!(harness.message().text.contains("已绑定"));
}

#[tokio::test]
async fn login_with_an_unknown_browser_is_refused() {
    let harness = Harness::new(|world| world.add("alice", 1, "success"));
    assert!(harness.controller.login_new(Some("Nope|".into())).is_err());
    assert!(!harness.controller.snapshot().busy);
}

#[tokio::test]
async fn reauth_warns_when_the_browser_authorized_another_account() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.add("bob", 2, "error");
        world.next_login = Some(1);
    });
    harness.refresh().await;
    harness
        .controller
        .reauthenticate("github.com".into(), "bob".into())
        .unwrap();
    harness.controller.wait_idle().await;
    let message = harness.message();
    assert!(message.is_error);
    assert!(message.text.contains("不是 bob"), "{message:?}");
}

#[tokio::test]
async fn reauth_uses_the_bound_browser() {
    let mut meta = meta_with(&[("bob", 2)]);
    meta.entry("github.com|bob").browser = Some(binding_of(&chrome()));
    let harness = Harness::with_meta(
        |world| {
            world.add("alice", 1, "success");
            world.add("bob", 2, "error");
            world.next_login = Some(2);
        },
        meta,
    );
    harness.refresh().await;
    harness
        .controller
        .reauthenticate("github.com".into(), "bob".into())
        .unwrap();
    harness.controller.wait_idle().await;
    assert_eq!(
        harness.platform.opened.lock().unwrap()[0].0,
        Some(chrome().id)
    );
    assert!(harness.message().text.contains("重新认证成功"));
    assert_eq!(harness.account("bob").state, AuthState::Authenticated);
}

#[tokio::test]
async fn rename_sync_relogs_then_removes_only_the_old_record() {
    let mut meta = MetaStore::in_memory();
    meta.entry("github.com|alice").browser = Some(binding_of(&chrome()));
    let harness = Harness::with_meta(
        |world| {
            world.add("alice", 1, "success");
            world.add("bob", 2, "success");
            world.github_names.insert(1, "alice-new".into());
            world.next_login = Some(1);
        },
        meta,
    );
    harness.refresh().await;
    assert!(harness.account("alice").renamed);

    harness
        .controller
        .sync_rename("github.com".into(), "alice".into())
        .unwrap();
    harness.controller.wait_idle().await;

    assert!(harness
        .calls()
        .contains(&"auth logout --hostname github.com --user alice".to_string()));
    let mut logins = harness.logins();
    logins.sort();
    assert_eq!(logins, vec!["alice-new", "bob"]);
    let renamed = harness.account("alice-new");
    assert!(!renamed.renamed);
    assert_eq!(renamed.browser.unwrap().id, chrome().id, "binding follows the account");
    assert!(harness.message().text.contains("已同步改名"));
}

#[tokio::test]
async fn rename_sync_keeps_the_old_record_when_another_account_was_authorized() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.add("bob", 2, "success");
        world.github_names.insert(1, "alice-new".into());
        world.next_login = Some(2);
    });
    harness.refresh().await;
    harness
        .controller
        .sync_rename("github.com".into(), "alice".into())
        .unwrap();
    harness.controller.wait_idle().await;

    assert!(!harness.calls().iter().any(|call| call.starts_with("auth logout")));
    assert!(harness.logins().contains(&"alice".to_string()));
    let message = harness.message();
    assert!(message.is_error);
    assert!(message.text.contains("未移除旧记录 alice"), "{message:?}");
}

#[tokio::test]
async fn rename_sync_skips_login_when_the_new_name_is_already_stored() {
    let harness = Harness::with_meta(
        |world| {
            world.add("alice-new", 1, "success");
            world.add("alice", 1, "success");
            world.github_names.insert(1, "alice-new".into());
        },
        meta_with(&[("alice", 1)]),
    );
    harness.refresh().await;
    assert!(harness.account("alice").renamed);
    harness
        .controller
        .sync_rename("github.com".into(), "alice".into())
        .unwrap();
    harness.controller.wait_idle().await;
    assert!(!harness.calls().iter().any(|call| call.starts_with("auth login")));
    assert_eq!(harness.logins(), vec!["alice-new"]);
}

#[tokio::test]
async fn cancelling_a_login_stops_it_and_blocks_nothing_afterwards() {
    let harness = Harness::new(|world| {
        world.add("alice", 1, "success");
        world.login_hangs = true;
    });
    harness.refresh().await;
    harness.controller.login_new(None).unwrap();

    // Wait for the device code to reach the UI.
    for _ in 0..100 {
        if harness
            .controller
            .snapshot()
            .login
            .is_some_and(|login| login.code == "ABCD-1234")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let login = harness.controller.snapshot().login.expect("login pending");
    assert_eq!(login.code, "ABCD-1234");
    assert_eq!(login.browser_label, DEFAULT_BROWSER_LABEL);
    assert_eq!(
        harness.platform.opened.lock().unwrap().clone(),
        vec![(None, "https://github.com/login/device".to_string())]
    );
    assert_eq!(harness.controller.refresh(), Err(BUSY.to_string()));

    harness.controller.cancel_login().unwrap();
    harness.controller.wait_idle().await;
    let snapshot = harness.controller.snapshot();
    assert!(snapshot.login.is_none());
    assert!(!snapshot.busy);
    assert_eq!(snapshot.message.text, "登录已取消");
    harness.refresh().await;
}

#[tokio::test]
async fn browser_binding_can_be_set_cleared_and_used_to_open_github() {
    let harness = Harness::new(|world| world.add("alice", 1, "success"));
    harness.refresh().await;
    harness
        .controller
        .set_account_browser("github.com".into(), "alice".into(), Some(chrome().id))
        .unwrap();
    assert_eq!(harness.account("alice").browser.unwrap().label, chrome().label);

    harness
        .controller
        .open_account_in_browser("github.com".into(), "alice".into())
        .unwrap();
    assert_eq!(
        harness.platform.opened.lock().unwrap().clone(),
        vec![(Some(chrome().id), "https://github.com/".to_string())]
    );

    harness
        .controller
        .set_account_browser("github.com".into(), "alice".into(), None)
        .unwrap();
    assert!(harness.account("alice").browser.is_none());
    assert!(harness
        .controller
        .set_account_browser("github.com".into(), "alice".into(), Some("Gone|".into()))
        .is_err());
}

#[tokio::test]
async fn a_missing_bound_browser_is_reported_instead_of_falling_back() {
    let mut meta = MetaStore::in_memory();
    meta.entry("github.com|alice").browser = Some(BrowserBinding {
        id: "Uninstalled|".into(),
        label: "Old Browser".into(),
    });
    let harness = Harness::with_meta(|world| world.add("alice", 1, "success"), meta);
    harness.refresh().await;
    let alice = harness.account("alice");
    assert_eq!(alice.browser.unwrap().label, "Old Browser");
    assert!(!alice.browser_available);
    assert!(harness
        .controller
        .open_account_in_browser("github.com".into(), "alice".into())
        .unwrap_err()
        .contains("Old Browser"));
    assert!(harness.platform.opened.lock().unwrap().is_empty());
}

/// Read-only check against the real gh and browser registry:
/// `cargo test real_gh_smoke -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "uses the installed GitHub CLI, network and registry"]
async fn real_gh_smoke() {
    struct NoOpen;
    impl BrowserPlatform for NoOpen {
        fn detect(&self) -> Vec<BrowserTarget> {
            crate::browsers::detect()
        }
        fn open(&self, _: Option<&BrowserTarget>, _: &str) -> Result<(), String> {
            Err("disabled in smoke test".into())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let controller = AuthController::new(
        crate::gh_process::GhCli,
        Arc::new(NoOpen),
        GhExecutableSettings::load_from(dir.path().join("settings.json")),
        MetaStore::in_memory(),
        Arc::new(|_| {}),
    );
    controller.refresh().unwrap();
    controller.wait_idle().await;
    let snapshot = controller.snapshot();
    println!("message: {}", snapshot.message.text);
    for browser in &snapshot.browsers {
        println!("browser: {} [{}]", browser.label, browser.id);
    }
    for account in &snapshot.accounts {
        println!(
            "account: active={} state={:?} identity_known={} renamed={} scopes={:?}",
            account.active,
            account.state,
            account.github_login.is_some(),
            account.renamed,
            account.scopes
        );
    }
    assert!(!snapshot.busy);
    assert!(!snapshot.message.is_error || !snapshot.accounts.is_empty());
}

#[test]
fn host_validation_rejects_url_tricks() {
    assert!(is_valid_host("github.com"));
    assert!(is_valid_host("ghe.example-corp.com"));
    assert!(!is_valid_host("evil.com/path"));
    assert!(!is_valid_host("user@evil.com"));
    assert!(!is_valid_host("-flag"));
    assert!(!is_valid_host(""));
}
