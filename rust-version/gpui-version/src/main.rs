//! GithubDeck for GPUI: a desktop shell over `ghdeck-core`.
//!
//! The window owns no async runtime. It renders the snapshot that the
//! controller thread last published and turns clicks into [`Command`]s, so
//! this UI and the Tauri one share all account logic.

// Build a GUI program: without this Windows opens a console window next to the
// app, because Rust defaults to the console subsystem.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod bridge;
mod platform;
mod view_model;

use bridge::{Bridge, Command, Shared};
use ghdeck_core::{
    AppStateSnapshot, AuthController, BrowserOption, GhCli, GhExecutableSettings, LoginView,
    MetaStore, UiMessage,
};
use gpui::{
    AnyElement, App, Application, Bounds, Context, Entity, InteractiveElement, MouseButton,
    SharedString, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use std::sync::{Arc, Mutex};
use view_model::{
    ConfirmAction, Dialog, ScopeRisk, Selection, StatusKind, display_name, high_risk_scopes,
    initials, scope_info, sorted_scopes, state_label,
};

const BG: u32 = 0xf6f8fa;
const CARD: u32 = 0xffffff;
const BORDER: u32 = 0xd0d7de;
const TEXT: u32 = 0x1f2328;
const MUTED: u32 = 0x656d76;
const GREEN: u32 = 0x1f883d;
const RED: u32 = 0xcf222e;
const BLUE: u32 = 0x0969da;
const AMBER_BG: u32 = 0xfff8c5;
const AMBER_BORDER: u32 = 0xd4a72c;
const RED_BG: u32 = 0xffebe9;
const RED_BORDER: u32 = 0xff8182;
const DARK: u32 = 0x24292f;

fn empty_snapshot() -> AppStateSnapshot {
    AppStateSnapshot {
        accounts: Vec::new(),
        busy: false,
        login: None,
        message: UiMessage {
            text: "准备就绪".into(),
            is_error: false,
        },
        gh_path: String::new(),
        browsers: Vec::new(),
        last_login_browser: None,
    }
}

struct GithubDeck {
    bridge: Bridge,
    platform: Arc<platform::DesktopPlatform>,
    seen_revision: u64,
    selection: Selection,
    dialog: Dialog,
}

impl GithubDeck {
    fn snapshot(&self) -> AppStateSnapshot {
        self.bridge.snapshot()
    }

    fn send(&self, command: Command) {
        self.bridge.send(command);
    }
}

impl Render for GithubDeck {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A notice from an awaited call (path validation) goes to the status bar.
        if let Some(notice) = self.bridge.take_notice() {
            // Path validation reports here; the controller owns other messages.
            let is_path_notice = !notice.is_error || notice.text.contains("gh.exe")
                || notice.text.contains("GitHub CLI")
                || notice.text.contains("路径")
                || notice.text.contains("默认");
            if is_path_notice {
                self.bridge.push_notice(notice);
            }
        }

        let revision = self.bridge.revision();
        if revision != self.seen_revision {
            self.seen_revision = revision;
            cx.notify();
        }

        let state = self.snapshot();
        self.selection.reconcile(&state.accounts);
        let entity = cx.entity();

        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .text_sm()
            .child(app_header())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .flex_1()
                    .min_h(px(0.0))
                    .px_5()
                    .py_4()
                    .child(page_header(&state, &entity))
                    .child(account_panel(&state, &self.selection, &entity)),
            );

        if let Some(login) = &state.login {
            root = root.child(login_panel(login, &entity));
        }

        root = root.child(bottom_bar(&state, &entity));
        if !self.dialog.is_none() {
            root = root.child(dialog_overlay(
                &self.dialog,
                &entity,
                self.platform.clone(),
                &state,
            ));
        }
        root.child(status_bar(&state.message))
    }
}

fn app_header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .h(px(48.0))
        .px_5()
        .bg(rgb(DARK))
        .child(
            div()
                .flex()
                .justify_center()
                .items_center()
                .size(px(24.0))
                .rounded_md()
                .bg(rgb(0x2da44e))
                .text_xs()
                .text_color(rgb(CARD))
                .child("GD"),
        )
        .child(
            div()
                .text_color(rgb(0xf0f6fc))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child("GithubDeck"),
        )
        .child(
            div()
                .ml_2()
                .text_xs()
                .text_color(rgb(0x8c959f))
                .child("GPUI 版"),
        )
}

fn page_header(state: &AppStateSnapshot, entity: &Entity<GithubDeck>) -> impl IntoElement {
    let busy = state.busy;
    let open_settings = entity.clone();
    let path = state.gh_path.clone();
    div()
        .flex()
        .justify_between()
        .items_start()
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("GitHub 账号"),
                )
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child("管理本机 GitHub CLI 中已认证的账号"),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(clickable(
                    button_surface("刷新", Tone::Plain, busy),
                    entity.clone(),
                    |this| this.send(Command::Refresh),
                ))
                .child(clickable(
                    button_surface("GitHub CLI 设置", Tone::Plain, busy),
                    open_settings,
                    move |this| {
                        this.dialog = Dialog::Settings {
                            path: path.clone(),
                            status: "选择 gh.exe 后点击保存校验".into(),
                            status_kind: StatusKind::Idle,
                            busy: false,
                        };
                    },
                )),
        )
}

fn account_panel(
    state: &AppStateSnapshot,
    selection: &Selection,
    entity: &Entity<GithubDeck>,
) -> impl IntoElement {
    let mut list = div()
        .id("accounts")
        .flex()
        .flex_col()
        .gap_2()
        .max_h(px(430.0))
        .overflow_y_scroll();

    if state.accounts.is_empty() {
        list = list.child(
            div()
                .flex()
                .justify_center()
                .items_center()
                .h(px(96.0))
                .w_full()
                .bg(rgb(BG))
                .border_1()
                .border_dashed()
                .border_color(rgb(BORDER))
                .rounded_md()
                .text_color(rgb(MUTED))
                .child("暂无账号，可点击“登录新账号”添加"),
        );
    } else {
        for account in &state.accounts {
            list = list.child(account_card(
                account,
                &state.browsers,
                selection,
                entity,
            ));
        }
    }

    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .bg(rgb(CARD))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_md()
        .child(
            div()
                .flex()
                .gap_2()
                .text_color(rgb(MUTED))
                .child(format!("已认证账号 · {} 个账号", state.accounts.len())),
        )
        .child(list)
}

fn account_card(
    account: &ghdeck_core::AccountView,
    browsers: &[BrowserOption],
    selection: &Selection,
    entity: &Entity<GithubDeck>,
) -> AnyElement {
    let expanded = selection.is_expanded(account);
    let name = display_name(account).to_string();
    let risky = high_risk_scopes(&account.scopes);
    let key = Selection::key(account);

    let header = div()
        .flex()
        .items_center()
        .gap_3()
        .p_2()
        .child(
            div()
                .flex()
                .justify_center()
                .items_center()
                .size(px(38.0))
                .rounded_full()
                .bg(rgb(DARK))
                .text_color(rgb(CARD))
                .font_weight(gpui::FontWeight::BOLD)
                .child(initials(&name)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(name.clone()),
                        )
                        .child(if account.renamed {
                            pill(
                                format!("本机记录：{}", account.login),
                                0x7d4e00,
                                AMBER_BG,
                                AMBER_BORDER,
                                false,
                            )
                            .into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                )
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child(format!(
                            "{} · {}{}",
                            account.host,
                            state_label(account.state),
                            match &account.browser {
                                Some(browser) if account.browser_available => {
                                    format!(" · {}", browser.label)
                                }
                                Some(browser) => format!(" · {}（未找到）", browser.label),
                                None => String::new(),
                            }
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(if risky.is_empty() {
                    div().into_any_element()
                } else {
                    pill("高风险权限", 0x82071e, RED_BG, RED_BORDER, true).into_any_element()
                })
                .child(if account.active {
                    pill("当前账号", 0x116329, 0xdafbe1, 0xaceebb, false).into_any_element()
                } else if account.state == ghdeck_core::AuthState::NeedsLogin {
                    pill("需重新认证", 0x633c01, AMBER_BG, AMBER_BORDER, false).into_any_element()
                } else {
                    pill(state_label(account.state), 0x57606a, BG, BORDER, false)
                        .into_any_element()
                }),
        );

    let mut card = div()
        .id(SharedString::from(format!("card-{key}")))
        .flex()
        .flex_col()
        .bg(rgb(CARD))
        .border_2()
        .border_color(rgb(if expanded { BLUE } else { BORDER }))
        .rounded_md()
        .child(header);

    if expanded {
        card = card.child(account_details(account, browsers, entity));
    }

    let toggle_key = key.clone();
    clickable(card, entity.clone(), move |this| {
        let accounts = this.snapshot().accounts;
        if let Some(account) = accounts
            .iter()
            .find(|candidate| Selection::key(candidate) == toggle_key)
        {
            this.selection.toggle(account);
        }
    })
    .into_any_element()
}

fn account_details(
    account: &ghdeck_core::AccountView,
    browsers: &[BrowserOption],
    entity: &Entity<GithubDeck>,
) -> gpui::Div {
    let risky = high_risk_scopes(&account.scopes);
    let mut details = div()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .pb_3()
        .border_t_1()
        .border_color(rgb(BORDER));

    if account.renamed {
        let host = account.host.clone();
        let login = account.login.clone();
        details = details.child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .gap_3()
                .p_2()
                .bg(rgb(AMBER_BG))
                .border_1()
                .border_color(rgb(AMBER_BORDER))
                .rounded_md()
                .child(format!(
                    "此账号已在 GitHub 改名为 {}，GitHub CLI 仍以 {} 保存。",
                    account.github_login.clone().unwrap_or_default(),
                    account.login
                ))
                .child(clickable(
                    button_surface("同步改名", Tone::Plain, false),
                    entity.clone(),
                    move |this| {
                        this.dialog = Dialog::Confirm {
                            title: "同步改名".into(),
                            body: format!(
                                "将重新授权，确认是同一个 GitHub 账号后，移除 GitHub CLI 中旧名 {login} 的记录。"
                            ),
                            action: ConfirmAction::SyncRename {
                                host: host.clone(),
                                login: login.clone(),
                            },
                        };
                    },
                )),
        );
    }

    if account.state == ghdeck_core::AuthState::NeedsLogin {
        let host = account.host.clone();
        let login = account.login.clone();
        details = details.child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .gap_3()
                .p_2()
                .bg(rgb(RED_BG))
                .border_1()
                .border_color(rgb(RED_BORDER))
                .rounded_md()
                .child("此账号的凭据已失效，需要重新认证。")
                .child(clickable(
                    button_surface("重新认证", Tone::Plain, false),
                    entity.clone(),
                    move |this| {
                        this.send(Command::Reauthenticate {
                            host: host.clone(),
                            login: login.clone(),
                        });
                    },
                )),
        );
    }

    details = details
        .child(div().text_color(rgb(MUTED)).child("权限"))
        .child(scope_chips(account));

    if !risky.is_empty() {
        details = details.child(danger_note(account, &risky));
    }

    details = details
        .child(div().text_color(rgb(MUTED)).child("浏览器"))
        .child(browser_row(account, entity))
        .child(browser_picker(account, browsers, entity));

    let switch_account = {
        let host = account.host.clone();
        let login = account.login.clone();
        let active = account.active;
        move || (host, login, active)
    };
    let (switch_host, switch_login, is_active) = switch_account();
    let host = account.host.clone();
    let login = account.login.clone();
    details.child(
        div()
            .flex()
            .justify_end()
            .gap_2()
            .child(clickable(
                button_surface("切换到所选账号", Tone::Primary, is_active),
                entity.clone(),
                move |this| {
                    this.send(Command::Switch {
                        host: switch_host.clone(),
                        login: switch_login.clone(),
                    });
                },
            ))
            .child(clickable(
                button_surface("登出所选账号", Tone::Danger, false),
                entity.clone(),
                move |this| {
                    this.dialog = Dialog::Confirm {
                        title: "登出所选账号".into(),
                        body: format!(
                            "确定要从本机 GitHub CLI 登出 {login}（{host}）吗？这只会移除本机认证配置，不会撤销 GitHub 端的 OAuth 授权。"
                        ),
                        action: ConfirmAction::Logout {
                            host: host.clone(),
                            login: login.clone(),
                        },
                    };
                },
            )),
    )
}

fn scope_chips(account: &ghdeck_core::AccountView) -> impl IntoElement {
    let mut row = div().flex().flex_wrap().gap_1();
    if account.scopes.is_empty() {
        return row.child(
            div()
                .text_color(rgb(MUTED))
                .child(if account.state == ghdeck_core::AuthState::NeedsLogin {
                    "凭据失效，无法读取权限"
                } else {
                    "未报告经典权限（可能是细粒度令牌或 GitHub App 令牌）"
                }),
        );
    }
    for scope in sorted_scopes(&account.scopes) {
        let info = scope_info(scope);
        let (color, background, border_color) = match info.risk {
            ScopeRisk::High => (0x82071e, RED_BG, RED_BORDER),
            ScopeRisk::Elevated => (0x7d4e00, AMBER_BG, AMBER_BORDER),
            ScopeRisk::Normal => (0x57606a, BG, BORDER),
        };
        row = row.child(pill(
            format!("{scope} · {}", info.description),
            color,
            background,
            border_color,
            info.risk == ScopeRisk::High,
        ));
    }
    row
}

fn danger_note(account: &ghdeck_core::AccountView, risky: &[&str]) -> impl IntoElement {
    let listing = risky
        .iter()
        .map(|scope| format!("{scope}（{}）", scope_info(scope).description))
        .collect::<Vec<_>>()
        .join("、");
    div()
        .text_color(rgb(0x82071e))
        .text_xs()
        .child(format!(
            "该令牌可以执行危险操作：{listing}。不需要时，可切换到此账号后执行 \
             gh auth refresh -h {} --remove-scopes {} 移除。",
            account.host,
            risky.join(",")
        ))
}

fn browser_row(
    account: &ghdeck_core::AccountView,
    entity: &Entity<GithubDeck>,
) -> impl IntoElement {
    let label = match &account.browser {
        Some(browser) if account.browser_available => browser.label.clone(),
        Some(browser) => format!("{}（未找到）", browser.label),
        None => "未绑定（系统默认浏览器）".into(),
    };
    let host = account.host.clone();
    let login = account.login.clone();
    let available = account.browser_available;
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .h(px(30.0))
                .px_2()
                .flex()
                .items_center()
                .bg(rgb(CARD))
                .border_1()
                .border_color(rgb(BORDER))
                .rounded_md()
                .child(label),
        )
        .child(clickable(
            button_surface("在此浏览器打开", Tone::Plain, !available),
            entity.clone(),
            move |this| {
                this.send(Command::OpenInBrowser {
                    host: host.clone(),
                    login: login.clone(),
                });
            },
        ))
}

fn browser_picker(
    account: &ghdeck_core::AccountView,
    browsers: &[BrowserOption],
    entity: &Entity<GithubDeck>,
) -> impl IntoElement {
    let current = account.browser.as_ref().map(|browser| browser.id.clone());
    let mut row = div().flex().flex_wrap().gap_1().items_center();
    row = row.child(div().text_color(rgb(MUTED)).w(px(88.0)).child("绑定浏览器"));

    let host = account.host.clone();
    let login = account.login.clone();
    row = row.child(clickable(
        pill("不绑定", 0x57606a, BG, BORDER, false),
        entity.clone(),
        move |this| {
            this.send(Command::SetBrowser {
                host: host.clone(),
                login: login.clone(),
                browser_id: None,
            });
        },
    ));

    for option in browsers.iter().take(8) {
        let selected = current.as_deref() == Some(option.id.as_str());
        let host = account.host.clone();
        let login = account.login.clone();
        let id = option.id.clone();
        row = row.child(clickable(
            pill(
                option.label.clone(),
                if selected { CARD } else { 0x57606a },
                if selected { BLUE } else { BG },
                if selected { BLUE } else { BORDER },
                selected,
            ),
            entity.clone(),
            move |this| {
                this.send(Command::SetBrowser {
                    host: host.clone(),
                    login: login.clone(),
                    browser_id: Some(id.clone()),
                });
            },
        ));
    }
    row
}

fn login_panel(login: &LoginView, entity: &Entity<GithubDeck>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .mx_5()
        .mb_3()
        .p_3()
        .bg(rgb(0xddf4ff))
        .border_1()
        .border_color(rgb(0x54aeff))
        .rounded_md()
        .child(
            div()
                .font_weight(gpui::FontWeight::BOLD)
                .child(match login.purpose {
                    "reauth" => format!(
                        "重新认证 {}",
                        login.target_login.clone().unwrap_or_default()
                    ),
                    "rename" => format!(
                        "同步改名：请授权原 {} 账号",
                        login.target_login.clone().unwrap_or_default()
                    ),
                    _ => "在浏览器中完成 GitHub 授权".into(),
                }),
        )
        .child(
            div()
                .text_color(rgb(MUTED))
                .child(if login.code.is_empty() {
                    "正在向 GitHub 申请一次性授权码…".to_string()
                } else {
                    format!(
                        "授权页已在 {} 打开，输入下方一次性授权码即可。应用不会读取或保存你的 Token。",
                        login.browser_label
                    )
                }),
        )
        .child(field("授权码", &login.code))
        .child(field("授权网址", &login.url))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(clickable(
                    button_surface("重新打开授权页", Tone::Plain, false),
                    entity.clone(),
                    |this| this.send(Command::ReopenAuthorization),
                ))
                .child(clickable(
                    button_surface("取消登录", Tone::Danger, false),
                    entity.clone(),
                    |this| this.send(Command::CancelLogin),
                )),
        )
}

fn field(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_color(rgb(MUTED))
                .w(px(72.0))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .h(px(30.0))
                .px_2()
                .flex()
                .items_center()
                .bg(rgb(CARD))
                .border_1()
                .border_color(rgb(BORDER))
                .rounded_md()
                .child(if value.is_empty() {
                    "…".to_string()
                } else {
                    value.to_string()
                }),
        )
}

fn bottom_bar(state: &AppStateSnapshot, entity: &Entity<GithubDeck>) -> impl IntoElement {
    let busy = state.busy;
    let browsers = state
        .browsers
        .iter()
        .map(|browser| view_model::BrowserChoice {
            id: browser.id.clone(),
            label: browser.label.clone(),
        })
        .collect::<Vec<_>>();
    let selected = state.last_login_browser.clone();
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_5()
        .pb_3()
        .child(
            div()
                .text_color(rgb(MUTED))
                .child(if busy { "操作进行中…" } else { "" }),
        )
        .child(div().flex_1())
        .child(clickable(
            button_surface("重新扫描浏览器", Tone::Plain, busy),
            entity.clone(),
            |this| this.send(Command::RescanBrowsers),
        ))
        .child(clickable(
            button_surface("登录新账号", Tone::Primary, busy),
            entity.clone(),
            move |this| {
                this.dialog = Dialog::Login {
                    browsers: browsers.clone(),
                    selected: selected.clone(),
                };
            },
        ))
}

fn dialog_overlay(
    dialog: &Dialog,
    entity: &Entity<GithubDeck>,
    platform: Arc<platform::DesktopPlatform>,
    state: &AppStateSnapshot,
) -> impl IntoElement {
    let _ = state;
    let content: AnyElement = match dialog {
        Dialog::None => div().into_any_element(),
        Dialog::Settings {
            path,
            status,
            status_kind,
            busy,
        } => settings_dialog(path, status, *status_kind, *busy, entity, platform),
        Dialog::Login { browsers, selected } => {
            login_dialog(browsers, selected.as_deref(), entity)
        }
        Dialog::Confirm { title, body, action } => confirm_dialog(title, body, action, entity),
    };
    div()
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .items_center()
        .bg(gpui::rgba(0x1f232880))
        .child(content)
}

fn settings_dialog(
    path: &str,
    status: &str,
    kind: StatusKind,
    busy: bool,
    entity: &Entity<GithubDeck>,
    _platform: Arc<platform::DesktopPlatform>,
) -> AnyElement {
    let color = match kind {
        StatusKind::Error => RED,
        StatusKind::Success => GREEN,
        StatusKind::Busy => BLUE,
        StatusKind::Idle => MUTED,
    };
    let path = path.to_string();
    let save_path = path.clone();
    dialog_frame()
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .child("GitHub CLI 设置"),
        )
        .child(
            div()
                .text_color(rgb(MUTED))
                .child("指定本机 gh.exe 路径。保存前会校验文件名与版本输出。"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .bg(rgb(CARD))
                .border_1()
                .border_color(rgb(BORDER))
                .rounded_md()
                .child(div().text_color(rgb(MUTED)).child("当前路径"))
                .child(
                    div()
                        .h(px(30.0))
                        .px_2()
                        .flex()
                        .items_center()
                        .bg(rgb(BG))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .rounded_md()
                        .child(path),
                )
                .child(div().flex().gap_2().child(clickable(
                    button_surface("选择 gh.exe", Tone::Plain, busy),
                    entity.clone(),
                    move |this| {
                        if let Some(picked) = platform::pick_executable() {
                            this.dialog = Dialog::Settings {
                                path: picked,
                                status: "已选择文件，点击保存进行校验".into(),
                                status_kind: StatusKind::Idle,
                                busy: false,
                            };
                        }
                    },
                )))
                .child(div().text_color(rgb(color)).child(status.to_string())),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(clickable(
                    button_surface("取消", Tone::Plain, false),
                    entity.clone(),
                    |this| this.dialog = Dialog::None,
                ))
                .child(clickable(
                    button_surface("保存", Tone::Primary, busy),
                    entity.clone(),
                    move |this| {
                        this.dialog = Dialog::Settings {
                            path: save_path.clone(),
                            status: "正在校验 GitHub CLI…".into(),
                            status_kind: StatusKind::Busy,
                            busy: true,
                        };
                        this.send(Command::SavePath {
                            path: save_path.clone(),
                        });
                    },
                ))
                .child(clickable(
                    button_surface("恢复默认", Tone::Plain, busy),
                    entity.clone(),
                    |this| this.send(Command::ResetPath),
                )),
        )
        .into_any_element()
}

fn login_dialog(
    browsers: &[view_model::BrowserChoice],
    selected: Option<&str>,
    entity: &Entity<GithubDeck>,
) -> AnyElement {
    let mut list = div().flex().flex_col().gap_1();
    list = list.child(login_choice(
        "系统默认浏览器",
        "",
        selected.unwrap_or("").is_empty(),
        entity.clone(),
    ));
    for browser in browsers {
        list = list.child(login_choice(
            &browser.label,
            &browser.id,
            selected == Some(browser.id.as_str()),
            entity.clone(),
        ));
    }

    dialog_frame()
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .child("登录新账号"),
        )
        .child(div().text_color(rgb(MUTED)).child(
            "选择要在哪个浏览器中完成授权。请确认该浏览器里登录的就是要添加的 GitHub 账号；登录成功后会自动绑定该浏览器。",
        ))
        .child(list)
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(clickable(
                    button_surface("取消", Tone::Plain, false),
                    entity.clone(),
                    |this| this.dialog = Dialog::None,
                ))
                .child(clickable(
                    button_surface("开始登录", Tone::Primary, false),
                    entity.clone(),
                    |this| {
                        let chosen = match &this.dialog {
                            Dialog::Login { selected, .. } => selected.clone(),
                            _ => None,
                        };
                        this.dialog = Dialog::None;
                        this.send(Command::LoginNew {
                            browser_id: chosen,
                        });
                    },
                )),
        )
        .into_any_element()
}

fn login_choice(
    label: &str,
    id: &str,
    checked: bool,
    entity: Entity<GithubDeck>,
) -> impl IntoElement {
    let id = id.to_string();
    clickable(
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_2()
            .bg(rgb(if checked { 0xf1f8ff } else { CARD }))
            .border_1()
            .border_color(rgb(if checked { BLUE } else { BORDER }))
            .rounded_md()
            .child(div().child(if checked { "◉" } else { "○" }))
            .child(div().child(label.to_string())),
        entity,
        move |this| {
            if let Dialog::Login { selected, .. } = &mut this.dialog {
                *selected = if id.is_empty() { None } else { Some(id.clone()) };
            }
        },
    )
}

fn confirm_dialog(
    title: &str,
    body: &str,
    action: &ConfirmAction,
    entity: &Entity<GithubDeck>,
) -> AnyElement {
    let action = action.clone();
    dialog_frame()
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .child(title.to_string()),
        )
        .child(div().text_color(rgb(MUTED)).child(body.to_string()))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(clickable(
                    button_surface("取消", Tone::Plain, false),
                    entity.clone(),
                    |this| this.dialog = Dialog::None,
                ))
                .child(clickable(
                    button_surface(
                        match action {
                            ConfirmAction::Logout { .. } => "确认登出",
                            ConfirmAction::SyncRename { .. } => "确认同步",
                        },
                        Tone::Danger,
                        false,
                    ),
                    entity.clone(),
                    move |this| {
                        this.dialog = Dialog::None;
                        match action.clone() {
                            ConfirmAction::Logout { host, login } => {
                                this.send(Command::Logout { host, login });
                            }
                            ConfirmAction::SyncRename { host, login } => {
                                this.send(Command::SyncRename { host, login });
                            }
                        }
                    },
                )),
        )
        .into_any_element()
}

fn dialog_frame() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .w(px(560.0))
        .bg(rgb(BG))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_lg()
}

fn status_bar(message: &UiMessage) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_4()
        .h(px(34.0))
        .bg(rgb(CARD))
        .border_t_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .text_xs()
                .text_color(rgb(if message.is_error { RED } else { GREEN }))
                .child("●"),
        )
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .text_color(rgb(if message.is_error { RED } else { TEXT }))
                .child(message.text.clone()),
        )
        .child(div().text_color(rgb(MUTED)).child("GitHub CLI"))
}

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Primary,
    Danger,
    Plain,
}

fn button_surface(label: &str, tone: Tone, disabled: bool) -> gpui::Div {
    let (color, background, border_color) = match (tone, disabled) {
        (_, true) => (0x8c959f, BG, 0xd8dee4),
        (Tone::Primary, false) => (CARD, GREEN, 0x1a7f37),
        (Tone::Danger, false) => (RED, CARD, BORDER),
        (Tone::Plain, false) => (TEXT, CARD, BORDER),
    };
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.0))
        .px_3()
        .rounded_md()
        .bg(rgb(background))
        .border_1()
        .border_color(rgb(border_color))
        .text_color(rgb(color))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(label.to_string())
}

fn pill(
    label: impl Into<String>,
    color: u32,
    background: u32,
    border_color: u32,
    bold: bool,
) -> gpui::Div {
    let element = div()
        .px_2()
        .rounded_full()
        .bg(rgb(background))
        .border_1()
        .border_color(rgb(border_color))
        .text_xs()
        .text_color(rgb(color))
        .child(label.into());
    if bold {
        element.font_weight(gpui::FontWeight::BOLD)
    } else {
        element
    }
}

/// Turns an element into a click target that mutates the window state.
///
/// The click stops propagating: a card's inner buttons (browser binding,
/// re-authentication, …) sit inside the card that toggles its details, so
/// without this a button press would also collapse the card the user is
/// working in. The pointing-hand cursor makes the target obvious.
fn clickable<E>(
    element: E,
    entity: Entity<GithubDeck>,
    action: impl Fn(&mut GithubDeck) + 'static,
) -> E
where
    E: InteractiveElement + Styled + Sized,
{
    let action = std::rc::Rc::new(action);
    element
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
            cx.stop_propagation();
            let action = action.clone();
            entity.update(cx, |this, cx| {
                action(this);
                cx.notify();
            });
        })
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let platform = Arc::new(platform::DesktopPlatform::new());
        let shared = Arc::new(Mutex::new(Shared::new(empty_snapshot())));

        let sink_shared = shared.clone();
        let controller = AuthController::new(
            GhCli,
            platform.clone(),
            GhExecutableSettings::load(),
            MetaStore::load(),
            Arc::new(move |snapshot: AppStateSnapshot| {
                let mut shared = sink_shared.lock().unwrap_or_else(|p| p.into_inner());
                shared.snapshot = snapshot;
                shared.revision += 1;
            }),
        );
        let bridge = Bridge::spawn(shared, controller);
        bridge.send(Command::Refresh);

        let bounds = Bounds::centered(None, size(px(880.0), px(720.0)), cx);
        let view = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("GithubDeck".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |_window, cx| {
                    cx.new(|_| GithubDeck {
                        bridge,
                        platform,
                        seen_revision: 0,
                        selection: Selection::default(),
                        dialog: Dialog::None,
                    })
                },
            )
            .unwrap();

        // The controller publishes snapshots from its own thread, and a plain
        // thread cannot notify a GPUI view. Watch the revision from a GPUI task
        // and redraw when it moves; that task ends when the window closes.
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(80))
                    .await;
                let updated = view.update(cx, |this, window, cx| {
                    let revision = this.bridge.revision();
                    if revision != this.seen_revision {
                        this.seen_revision = revision;
                        window.refresh();
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.activate(true);
    });
}
