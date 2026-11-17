//! Bridges the GPUI window (which owns no async runtime) and the controller
//! (which needs one). UI actions become commands on a queue; one thread owns
//! the tokio runtime and the controller, so `tokio::spawn` inside the
//! controller always has a runtime.

use ghdeck_core::{AppStateSnapshot, AuthController, GhCli, UiMessage};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub type Controller = AuthController<GhCli>;

/// Everything the window can ask the controller to do.
#[derive(Debug, Clone)]
pub enum Command {
    Refresh,
    Switch { host: String, login: String },
    Logout { host: String, login: String },
    LoginNew { browser_id: Option<String> },
    Reauthenticate { host: String, login: String },
    SyncRename { host: String, login: String },
    CancelLogin,
    ReopenAuthorization,
    SetBrowser { host: String, login: String, browser_id: Option<String> },
    OpenInBrowser { host: String, login: String },
    RescanBrowsers,
    SavePath { path: String },
    ResetPath,
    #[allow(dead_code)]
    Shutdown,
}

pub struct Shared {
    pub snapshot: AppStateSnapshot,
    pub revision: u64,
    /// Message produced by an awaited call (validation etc.) for the status bar.
    pub notice: Option<UiMessage>,
}

impl Shared {
    pub fn new(snapshot: AppStateSnapshot) -> Self {
        Self {
            snapshot,
            revision: 1,
            notice: None,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

pub struct Bridge {
    shared: Arc<Mutex<Shared>>,
    commands: Sender<Command>,
}

impl Bridge {
    pub fn spawn(shared: Arc<Mutex<Shared>>, controller: Controller) -> Self {
        let (commands, receiver) = std::sync::mpsc::channel::<Command>();
        let worker_shared = shared.clone();
        std::thread::Builder::new()
            .name("ghdeck-core".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(_) => return,
                };
                runtime.block_on(pump(receiver, controller, worker_shared));
            })
            .expect("controller thread");
        Self { shared, commands }
    }

    pub fn snapshot(&self) -> AppStateSnapshot {
        guard(&self.shared).snapshot.clone()
    }

    pub fn revision(&self) -> u64 {
        guard(&self.shared).revision()
    }

    /// Takes the pending notice, if a completed call left one. Callers merge
    /// it into the status bar; the controller emits its own messages too.
    pub fn take_notice(&self) -> Option<UiMessage> {
        guard(&self.shared).notice.take()
    }

    /// Shows a notice in the status bar of the shared snapshot.
    pub fn push_notice(&self, notice: UiMessage) {
        let mut shared = guard(&self.shared);
        shared.snapshot.message = notice;
        shared.revision += 1;
    }

    pub fn send(&self, command: Command) {
        // A closed channel only happens while shutting down.
        let _ = self.commands.send(command);
    }
}

fn guard<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn pump(receiver: Receiver<Command>, controller: Controller, shared: Arc<Mutex<Shared>>) {
    // Commands arrive on a std channel; poll it without blocking the runtime.
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(25));
    loop {
        ticker.tick().await;
        loop {
            let command = match receiver.try_recv() {
                Ok(command) => command,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            };
            match command {
                Command::Shutdown => {
                    controller.shutdown().await;
                    return;
                }
                Command::SavePath { path } => {
                    let result = controller.save_gh_path(path).await;
                    let mut guard = guard(&shared);
                    guard.notice = Some(match result {
                        Ok(_) => UiMessage {
                            text: "GitHub CLI 路径已保存".into(),
                            is_error: false,
                        },
                        Err(error) => UiMessage {
                            text: error,
                            is_error: true,
                        },
                    });
                    guard.revision += 1;
                }
                Command::ResetPath => {
                    let result = controller.reset_gh_path().await;
                    let mut guard = guard(&shared);
                    guard.notice = Some(match result {
                        Ok(_) => UiMessage {
                            text: "已恢复默认 GitHub CLI 路径".into(),
                            is_error: false,
                        },
                        Err(error) => UiMessage {
                            text: error,
                            is_error: true,
                        },
                    });
                    guard.revision += 1;
                }
                // Everything else is a plain Result and is dispatched inline.
                other => {
                    let result = match other {
                        Command::Refresh => controller.refresh(),
                        Command::Switch { host, login } => controller.switch_account(host, login),
                        Command::Logout { host, login } => controller.logout(host, login),
                        Command::LoginNew { browser_id } => controller.login_new(browser_id),
                        Command::Reauthenticate { host, login } => {
                            controller.reauthenticate(host, login)
                        }
                        Command::SyncRename { host, login } => controller.sync_rename(host, login),
                        Command::CancelLogin => controller.cancel_login(),
                        Command::ReopenAuthorization => controller.reopen_authorization_page(),
                        Command::SetBrowser {
                            host,
                            login,
                            browser_id,
                        } => controller.set_account_browser(host, login, browser_id),
                        Command::OpenInBrowser { host, login } => {
                            controller.open_account_in_browser(host, login)
                        }
                        Command::RescanBrowsers => {
                            controller.rescan_browsers();
                            Ok(())
                        }
                        Command::SavePath { .. }
                        | Command::ResetPath
                        | Command::Shutdown => unreachable!("handled above"),
                    };
                    if let Err(error) = result {
                        let mut guard = guard(&shared);
                        guard.notice = Some(UiMessage {
                            text: error,
                            is_error: true,
                        });
                        guard.revision += 1;
                    }
                }
            }
        }
    }
}
