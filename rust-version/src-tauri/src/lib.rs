mod auth_parser;
mod controller;
mod gh_process;
mod models;
mod settings;

use controller::AuthController;
use gh_process::GhProcess;
use settings::GhExecutableSettings;
use std::sync::Arc;
use tauri::{AppHandle, Listener, Manager, State};

struct AppState {
    controller: AuthController,
}

#[tauri::command]
async fn get_state(
    state: State<'_, Arc<AppState>>,
) -> Result<controller::AppStateSnapshot, String> {
    Ok(state.controller.snapshot().await)
}

#[tauri::command]
async fn refresh_accounts(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.controller.refresh(app).await
}

#[tauri::command]
async fn switch_account(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    host: String,
    login: String,
) -> Result<(), String> {
    state.controller.switch_account(app, host, login).await
}

#[tauri::command]
async fn logout_account(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    host: String,
    login: String,
) -> Result<(), String> {
    state.controller.logout(app, host, login).await
}

#[tauri::command]
async fn login_account(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.controller.login(app).await
}

#[tauri::command]
async fn cancel_login(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.controller.cancel(app).await
}

#[tauri::command]
async fn get_gh_path(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(state.controller.get_gh_path().await)
}

#[tauri::command]
async fn validate_gh_path(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<settings::ValidationResult, String> {
    Ok(state.controller.validate_gh_path(path).await)
}

#[tauri::command]
async fn save_gh_path(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<String, String> {
    state.controller.save_gh_path(app, path).await
}

#[tauri::command]
async fn reset_gh_path(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<String, String> {
    state.controller.reset_gh_path(app).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let settings = GhExecutableSettings::load();
            let process = GhProcess::new(settings);
            let controller = AuthController::new(process);
            let state = Arc::new(AppState {
                controller: controller.clone(),
            });
            app.manage(state.clone());

            let app_handle = app.handle().clone();
            let controller_for_finished = controller.clone();
            app.listen("gh-finished", move |event| {
                let app = app_handle.clone();
                let controller = controller_for_finished.clone();
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<FinishedPayload>(&payload) {
                        controller
                            .on_process_finished(app, data.operation, data.success, data.output)
                            .await;
                    }
                });
            });

            let app_handle = app.handle().clone();
            let controller_for_code = controller.clone();
            app.listen("authorization-code", move |event| {
                let app = app_handle.clone();
                let controller = controller_for_code.clone();
                let payload = event.payload().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Ok(data) = serde_json::from_str::<CodePayload>(&payload) {
                        controller.set_authorization_code(&app, data.code).await;
                    }
                });
            });

            let controller_for_boot = controller;
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = controller_for_boot.refresh(app_handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            refresh_accounts,
            switch_account,
            logout_account,
            login_account,
            cancel_login,
            get_gh_path,
            validate_gh_path,
            save_gh_path,
            reset_gh_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinishedPayload {
    operation: String,
    success: bool,
    output: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodePayload {
    code: String,
}
