//! GithubDeck core: everything that talks to GitHub CLI and keeps local state.
//!
//! Both desktop UIs (Tauri and GPUI) are thin shells over [`AuthController`]:
//! they create one with a [`StateSink`] and render whatever snapshot arrives.

pub mod account_meta;
pub mod auth_parser;
pub mod browsers;
pub mod controller;
pub mod gh_process;
pub mod models;
pub mod settings;

pub use account_meta::{AccountMeta, BrowserBinding, MetaStore};
pub use auth_parser::{parse_auth_status, parse_identity, AuthStatusError, GithubIdentity};
pub use browsers::{detect, launch, BrowserTarget, InstalledBrowser};
pub use controller::{
    AccountView, AppStateSnapshot, AuthController, BrowserOption, BrowserPlatform, LoginView,
    StateSink, UiMessage,
};
pub use gh_process::{GhCli, GhRunner, RunOutput, RunRequest, RunStatus};
pub use models::{account_key, Account, AuthState};
pub use settings::{
    validate_executable, GhExecutableSettings, ValidationResult, DEFAULT_GH_EXECUTABLE,
};
