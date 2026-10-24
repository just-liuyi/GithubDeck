mod account_meta;
mod auth_parser;
mod browsers;
mod controller;
mod gh_process;
mod models;
mod settings;

use account_meta::MetaStore;
use browsers::BrowserTarget;
use controller::{AppStateSnapshot, AuthController, BrowserPlatform};
use gh_process::GhCli;
use settings::GhExecutableSettings;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

type Controller = AuthController<GhCli>;

struct DesktopPlatform {
    app: AppHandle,
}

impl BrowserPlatform for DesktopPlatform {
    fn detect(&self) -> Vec<BrowserTarget> {
        browsers::detect()
    }

    fn open(&self, target: Option<&BrowserTarget>, url: &str) -> Result<(), String> {
        match target {
            Some(target) => browsers::launch(target, url),
            None => self
                .app
                .opener()
                .open_url(url, None::<String>)
                .map_err(|_| "无法打开系统默认浏览器".to_string()),
        }
    }
}

#[tauri::command]
async fn get_state(controller: State<'_, Controller>) -> Result<AppStateSnapshot, String> {
    Ok(controller.snapshot())
}

#[tauri::command]
async fn refresh_accounts(controller: State<'_, Controller>) -> Result<(), String> {
    controller.refresh()
}

#[tauri::command]
async fn switch_account(
    controller: State<'_, Controller>,
    host: String,
    login: String,
) -> Result<(), String> {
    controller.switch_account(host, login)
}

#[tauri::command]
async fn logout_account(
    controller: State<'_, Controller>,
    host: String,
    login: String,
) -> Result<(), String> {
    controller.logout(host, login)
}

#[tauri::command]
async fn login_account(
    controller: State<'_, Controller>,
    browser_id: Option<String>,
) -> Result<(), String> {
    controller.login_new(browser_id)
}

#[tauri::command]
async fn reauthenticate_account(
    controller: State<'_, Controller>,
    host: String,
    login: String,
) -> Result<(), String> {
    controller.reauthenticate(host, login)
}

#[tauri::command]
async fn sync_account_rename(
    controller: State<'_, Controller>,
    host: String,
    login: String,
) -> Result<(), String> {
    controller.sync_rename(host, login)
}

#[tauri::command]
async fn cancel_login(controller: State<'_, Controller>) -> Result<(), String> {
    controller.cancel_login()
}

#[tauri::command]
async fn reopen_authorization_page(controller: State<'_, Controller>) -> Result<(), String> {
    controller.reopen_authorization_page()
}

#[tauri::command]
async fn set_account_browser(
    controller: State<'_, Controller>,
    host: String,
    login: String,
    browser_id: Option<String>,
) -> Result<(), String> {
    controller.set_account_browser(host, login, browser_id)
}

#[tauri::command]
async fn open_account_in_browser(
    controller: State<'_, Controller>,
    host: String,
    login: String,
) -> Result<(), String> {
    controller.open_account_in_browser(host, login)
}

#[tauri::command]
async fn rescan_browsers(controller: State<'_, Controller>) -> Result<(), String> {
    controller.rescan_browsers();
    Ok(())
}

#[tauri::command]
async fn get_gh_path(controller: State<'_, Controller>) -> Result<String, String> {
    Ok(controller.gh_path())
}

/// Native file dialog; the page never sees anything but the chosen path.
#[tauri::command]
async fn pick_gh_executable(app: AppHandle) -> Result<Option<String>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("选择 gh.exe")
        .add_filter("GitHub CLI (gh.exe)", &["exe"])
        .pick_file(move |path| {
            let _ = sender.send(path);
        });
    let picked = receiver
        .await
        .map_err(|_| "无法打开文件选择对话框".to_string())?;
    Ok(picked
        .and_then(|path| path.into_path().ok())
        .map(|path| path.display().to_string()))
}

#[tauri::command]
async fn save_gh_path(controller: State<'_, Controller>, path: String) -> Result<String, String> {
    controller.save_gh_path(path).await
}

#[tauri::command]
async fn reset_gh_path(controller: State<'_, Controller>) -> Result<String, String> {
    controller.reset_gh_path().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let emitter = app.handle().clone();
            let controller = Controller::new(
                GhCli,
                Arc::new(DesktopPlatform { app: handle }),
                GhExecutableSettings::load(),
                MetaStore::load(),
                Arc::new(move |snapshot| {
                    let _ = emitter.emit("app-state", snapshot);
                }),
            );
            app.manage(controller.clone());
            tauri::async_runtime::spawn(async move {
                let _ = controller.refresh();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh_accounts,
            switch_account,
            logout_account,
            login_account,
            reauthenticate_account,
            sync_account_rename,
            cancel_login,
            reopen_authorization_page,
            set_account_browser,
            open_account_in_browser,
            rescan_browsers,
            get_gh_path,
            pick_gh_executable,
            save_gh_path,
            reset_gh_path
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // A pending `gh auth login` must not outlive the window: the user
            // could still finish authorizing it in the browser later.
            if let Some(controller) = handle.try_state::<Controller>() {
                let controller = controller.inner().clone();
                tauri::async_runtime::block_on(controller.shutdown());
            }
        }
    });
}
