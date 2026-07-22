use crate::auth_parser::{parse_auth_status, AuthStatusError};
use crate::gh_process::GhProcess;
use crate::models::Account;
use crate::settings::{validate_executable, ValidationResult};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiMessage {
    pub text: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateSnapshot {
    pub accounts: Vec<Account>,
    pub busy: bool,
    pub login_active: bool,
    pub authorization_code: String,
    pub message: UiMessage,
    pub gh_path: String,
}

struct ControllerInner {
    process: GhProcess,
    busy: bool,
    login_active: bool,
    cancel_requested: bool,
    confirmation: String,
    authorization_code: String,
    accounts_snapshot: Vec<Account>,
    message: UiMessage,
}

#[derive(Clone)]
pub struct AuthController {
    inner: Arc<Mutex<ControllerInner>>,
}

impl AuthController {
    pub fn new(process: GhProcess) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ControllerInner {
                process,
                busy: false,
                login_active: false,
                cancel_requested: false,
                confirmation: String::new(),
                authorization_code: String::new(),
                accounts_snapshot: Vec::new(),
                message: UiMessage {
                    text: "准备就绪".into(),
                    is_error: false,
                },
            })),
        }
    }

    pub async fn snapshot(&self) -> AppStateSnapshot {
        let inner = self.inner.lock().await;
        AppStateSnapshot {
            accounts: inner.accounts_snapshot.clone(),
            busy: inner.busy,
            login_active: inner.login_active,
            authorization_code: inner.authorization_code.clone(),
            message: inner.message.clone(),
            gh_path: inner.process.settings_path().await,
        }
    }

    pub async fn refresh(&self, app: AppHandle) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            if inner.busy {
                return Err("当前有操作正在进行".into());
            }
            if !inner.process.executable_available().await {
                inner.message = UiMessage {
                    text: "未找到 GitHub CLI，请在设置中指定有效的 gh.exe".into(),
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
                return Err(inner.message.text.clone());
            }
            inner.busy = true;
            inner.message = UiMessage {
                text: "正在刷新账号…".into(),
                is_error: false,
            };
            self.emit_state(&app, &inner).await;
        }

        let process = {
            let inner = self.inner.lock().await;
            inner.process.clone()
        };
        if let Err(error) = process.refresh(app.clone()).await {
            let mut inner = self.inner.lock().await;
            inner.busy = false;
            inner.message = UiMessage {
                text: error.clone(),
                is_error: true,
            };
            self.emit_state(&app, &inner).await;
            return Err(error);
        }
        Ok(())
    }

    pub async fn switch_account(
        &self,
        app: AppHandle,
        host: String,
        login: String,
    ) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            if inner.busy {
                return Err("当前有操作正在进行".into());
            }
            if !Self::trusted(&inner.accounts_snapshot, &host, &login) {
                inner.message = UiMessage {
                    text: "账号列表已变化，请刷新后重试".into(),
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
                return Err(inner.message.text.clone());
            }
            if !inner.process.executable_available().await {
                inner.message = UiMessage {
                    text: "未找到 GitHub CLI，请在设置中指定有效的 gh.exe".into(),
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
                return Err(inner.message.text.clone());
            }
            inner.busy = true;
            inner.message = UiMessage {
                text: format!("正在切换到 {login}…"),
                is_error: false,
            };
            self.emit_state(&app, &inner).await;
        }

        let process = {
            let inner = self.inner.lock().await;
            inner.process.clone()
        };
        if let Err(error) = process.switch(app.clone(), &host, &login).await {
            let mut inner = self.inner.lock().await;
            inner.busy = false;
            inner.message = UiMessage {
                text: error.clone(),
                is_error: true,
            };
            self.emit_state(&app, &inner).await;
            return Err(error);
        }
        Ok(())
    }

    pub async fn logout(&self, app: AppHandle, host: String, login: String) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            if inner.busy {
                return Err("当前有操作正在进行".into());
            }
            if !Self::trusted(&inner.accounts_snapshot, &host, &login) {
                inner.message = UiMessage {
                    text: "账号列表已变化，请刷新后重试".into(),
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
                return Err(inner.message.text.clone());
            }
            if !inner.process.executable_available().await {
                inner.message = UiMessage {
                    text: "未找到 GitHub CLI，请在设置中指定有效的 gh.exe".into(),
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
                return Err(inner.message.text.clone());
            }
            inner.busy = true;
            inner.message = UiMessage {
                text: format!("正在登出 {login}…"),
                is_error: false,
            };
            self.emit_state(&app, &inner).await;
        }

        let process = {
            let inner = self.inner.lock().await;
            inner.process.clone()
        };
        if let Err(error) = process.logout(app.clone(), &host, &login).await {
            let mut inner = self.inner.lock().await;
            inner.busy = false;
            inner.message = UiMessage {
                text: error.clone(),
                is_error: true,
            };
            self.emit_state(&app, &inner).await;
            return Err(error);
        }
        Ok(())
    }

    pub async fn login(&self, app: AppHandle) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            if inner.busy {
                return Err("当前有操作正在进行".into());
            }
            if !inner.process.executable_available().await {
                inner.message = UiMessage {
                    text: "未找到 GitHub CLI，请在设置中指定有效的 gh.exe".into(),
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
                return Err(inner.message.text.clone());
            }
            inner.cancel_requested = false;
            inner.authorization_code.clear();
            inner.busy = true;
            inner.login_active = true;
            inner.message = UiMessage {
                text: "正在生成授权码并打开 GitHub 官方授权页面…".into(),
                is_error: false,
            };
            self.emit_state(&app, &inner).await;
        }

        let process = {
            let inner = self.inner.lock().await;
            inner.process.clone()
        };
        if let Err(error) = process.login(app.clone()).await {
            let mut inner = self.inner.lock().await;
            inner.login_active = false;
            inner.busy = false;
            inner.message = UiMessage {
                text: error.clone(),
                is_error: true,
            };
            self.emit_state(&app, &inner).await;
            return Err(error);
        }
        Ok(())
    }

    pub async fn cancel(&self, app: AppHandle) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        if !inner.login_active {
            return Err("当前没有可取消的登录".into());
        }
        if !inner.process.cancel().await {
            return Err("当前没有可取消的登录".into());
        }
        inner.cancel_requested = true;
        inner.authorization_code.clear();
        inner.login_active = false;
        inner.message = UiMessage {
            text: "正在取消登录…".into(),
            is_error: false,
        };
        self.emit_state(&app, &inner).await;
        Ok(())
    }

    pub async fn set_authorization_code(&self, app: &AppHandle, code: String) {
        let mut inner = self.inner.lock().await;
        if inner.login_active {
            inner.authorization_code = code;
            self.emit_state(app, &inner).await;
        }
    }

    pub async fn on_process_finished(
        &self,
        app: AppHandle,
        operation: String,
        success: bool,
        output: String,
    ) {
        if operation == "status" {
            self.finish_refresh(app, success, output.as_bytes()).await;
            return;
        }

        {
            let mut inner = self.inner.lock().await;
            if operation == "login" {
                inner.authorization_code.clear();
                inner.login_active = false;
            }

            if !success {
                inner.busy = false;
                if operation == "login" && inner.cancel_requested {
                    inner.cancel_requested = false;
                    inner.message = UiMessage {
                        text: "登录已取消".into(),
                        is_error: false,
                    };
                    self.emit_state(&app, &inner).await;
                    return;
                }
                let action = match operation.as_str() {
                    "switch" => "切换账号",
                    "logout" => "登出账号",
                    "login" => "登录",
                    _ => "操作",
                };
                inner.message = UiMessage {
                    text: format!("{action}失败，请检查 GitHub CLI 配置后重试"),
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
                return;
            }

            inner.cancel_requested = false;
            inner.confirmation = match operation.as_str() {
                "switch" => "账号切换成功".into(),
                "logout" => "账号登出成功".into(),
                "login" => "登录成功".into(),
                _ => "操作成功".into(),
            };
            inner.message = UiMessage {
                text: "操作完成，正在刷新确认…".into(),
                is_error: false,
            };
            self.emit_state(&app, &inner).await;
        }

        let process = {
            let inner = self.inner.lock().await;
            inner.process.clone()
        };
        if process.refresh(app.clone()).await.is_err() {
            let mut inner = self.inner.lock().await;
            inner.confirmation.clear();
            inner.busy = false;
            inner.message = UiMessage {
                text: "操作完成，但无法刷新确认".into(),
                is_error: true,
            };
            self.emit_state(&app, &inner).await;
        }
    }

    async fn finish_refresh(&self, app: AppHandle, success: bool, output: &[u8]) {
        let mut inner = self.inner.lock().await;
        match parse_auth_status(output) {
            Ok(accounts) => {
                inner.accounts_snapshot = accounts.clone();
                if !success && !accounts.is_empty() {
                    inner.message = UiMessage {
                        text: "已显示可用账号，部分账号可能需要重新认证".into(),
                        is_error: true,
                    };
                } else {
                    let text = if !inner.confirmation.is_empty() {
                        inner.confirmation.clone()
                    } else if accounts.is_empty() {
                        "尚未登录 GitHub 账号".into()
                    } else {
                        "账号列表已刷新".into()
                    };
                    inner.message = UiMessage {
                        text,
                        is_error: false,
                    };
                }
                inner.confirmation.clear();
                inner.busy = false;
                self.emit_state(&app, &inner).await;
            }
            Err(error) => {
                inner.confirmation.clear();
                inner.accounts_snapshot.clear();
                inner.busy = false;
                inner.message = UiMessage {
                    text: match error {
                        AuthStatusError::InvalidJson => "GitHub CLI 返回了无法解析的状态".into(),
                        AuthStatusError::MissingHosts => "GitHub CLI 状态中缺少 hosts".into(),
                    },
                    is_error: true,
                };
                self.emit_state(&app, &inner).await;
            }
        }
    }

    pub async fn get_gh_path(&self) -> String {
        let inner = self.inner.lock().await;
        inner.process.settings_path().await
    }

    pub async fn validate_gh_path(&self, path: String) -> ValidationResult {
        validate_executable(&path)
    }

    pub async fn save_gh_path(&self, app: AppHandle, path: String) -> Result<String, String> {
        let result = validate_executable(&path);
        if !result.valid {
            return Err(result.message);
        }
        let mut inner = self.inner.lock().await;
        inner.process.set_settings_path(&path).await?;
        let saved = inner.process.settings_path().await;
        inner.message = UiMessage {
            text: "GitHub CLI 路径已保存".into(),
            is_error: false,
        };
        self.emit_state(&app, &inner).await;
        Ok(saved)
    }

    pub async fn reset_gh_path(&self, app: AppHandle) -> Result<String, String> {
        let mut inner = self.inner.lock().await;
        inner.process.reset_settings_path().await?;
        let path = inner.process.settings_path().await;
        let result = validate_executable(&path);
        if !result.valid {
            return Err(result.message);
        }
        inner.message = UiMessage {
            text: "已恢复默认 GitHub CLI 路径".into(),
            is_error: false,
        };
        self.emit_state(&app, &inner).await;
        Ok(path)
    }

    pub async fn shutdown(&self) {
        let inner = self.inner.lock().await;
        inner.process.shutdown().await;
    }

    fn trusted(accounts: &[Account], host: &str, login: &str) -> bool {
        accounts
            .iter()
            .any(|account| account.host == host && account.login == login)
    }

    async fn emit_state(&self, app: &AppHandle, inner: &ControllerInner) {
        let snapshot = AppStateSnapshot {
            accounts: inner.accounts_snapshot.clone(),
            busy: inner.busy,
            login_active: inner.login_active,
            authorization_code: inner.authorization_code.clone(),
            message: inner.message.clone(),
            gh_path: inner.process.settings_path().await,
        };
        let _ = app.emit("app-state", snapshot);
    }
}
