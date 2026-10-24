import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import AccountCard from "./components/AccountCard";
import LoginDialog from "./components/LoginDialog";
import SettingsDialog, { GITHUB_CLI_RELEASES_URL } from "./components/SettingsDialog";
import { type Account, type AppState, type LoginView, accountKey, displayName } from "./types";
import "./App.css";

const emptyState: AppState = {
  accounts: [],
  busy: false,
  login: null,
  message: { text: "准备就绪", isError: false },
  ghPath: "",
  browsers: [],
  lastLoginBrowser: null,
};

function loginTitle(login: LoginView): string {
  switch (login.purpose) {
    case "reauth":
      return `重新认证 ${login.targetLogin ?? ""}`;
    case "rename":
      return `同步改名：请授权原 ${login.targetLogin ?? ""} 账号`;
    default:
      return "在浏览器中完成 GitHub 授权";
  }
}

export default function App() {
  const [state, setState] = useState<AppState>(emptyState);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [loginDialogOpen, setLoginDialogOpen] = useState(false);

  const selectedAccount = useMemo(
    () => state.accounts.find((account) => accountKey(account) === selectedKey) ?? null,
    [selectedKey, state.accounts],
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    (async () => {
      const stop = await listen<AppState>("app-state", (event) => setState(event.payload));
      if (disposed) {
        stop();
        return;
      }
      unlisten = stop;
      try {
        setState(await invoke<AppState>("get_state"));
      } catch {
        // The backend pushes a state on every change; keep the placeholder.
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  function showMessage(text: string, isError: boolean) {
    setState((current) => ({ ...current, message: { text, isError } }));
  }

  async function runCommand(command: string, args?: Record<string, unknown>) {
    try {
      await invoke(command, args);
    } catch (error) {
      showMessage(typeof error === "string" ? error : "操作失败", true);
    }
  }

  async function copyText(text: string, successMessage: string) {
    try {
      await navigator.clipboard.writeText(text);
      showMessage(successMessage, false);
    } catch {
      showMessage("无法访问系统剪贴板，请稍后重试", true);
    }
  }

  async function openGithubCliReleases() {
    try {
      await openUrl(GITHUB_CLI_RELEASES_URL);
    } catch {
      showMessage("无法打开 GitHub CLI 官方发布页，请稍后重试", true);
    }
  }

  function target(account: Account) {
    return { host: account.host, login: account.login };
  }

  function confirmLogout(account: Account) {
    const ok = window.confirm(
      `确定要从本机 GitHub CLI 登出 ${displayName(account)}（${account.host}）吗？\n\n这只会移除本机保存的认证配置，不会撤销 GitHub 端的 OAuth 授权。`,
    );
    if (ok) void runCommand("logout_account", target(account));
  }

  function confirmSyncRename(account: Account) {
    const ok = window.confirm(
      `同步改名：GitHub CLI 将以新名称 ${account.githubLogin} 保存这个账号。\n\n` +
        `1. 将在${account.browser ? ` ${account.browser.label} ` : "系统默认浏览器"}中重新授权，请确认浏览器里登录的是该账号；\n` +
        `2. 确认授权的是同一个 GitHub 账号后，移除旧记录 ${account.login}。\n\n` +
        "如果授权的是其他账号，旧记录会保留。",
    );
    if (ok) void runCommand("sync_account_rename", target(account));
  }

  const { busy, login } = state;
  const switchEnabled = !busy && selectedAccount !== null && !selectedAccount.active;
  const logoutEnabled = !busy && selectedAccount !== null;

  return (
    <div className="main-container">
      <header className="app-header">
        <img className="app-mark" src="/favicon.png" alt="" />
        <div className="app-title">GithubDeck</div>
      </header>

      <main className="content">
        <section className="page-header">
          <div>
            <h1 className="page-title">GitHub 账号</h1>
            <p className="page-description">管理本机 GitHub CLI 中已认证的账号</p>
          </div>
          <div className="page-actions">
            <button type="button" disabled={busy} onClick={() => runCommand("refresh_accounts")}>
              刷新
            </button>
            <button type="button" disabled={busy} onClick={() => setSettingsOpen(true)}>
              GitHub CLI 设置
            </button>
          </div>
        </section>

        <section className="account-section">
          <div className="account-heading">
            <span className="account-section-label">已认证账号</span>
            <span className="account-count-label">{state.accounts.length} 个账号</span>
          </div>
          <div className={`account-list ${busy ? "disabled" : ""}`} role="list">
            {state.accounts.length === 0 ? (
              <div className="empty-accounts">暂无账号，可点击“登录新账号”添加</div>
            ) : (
              state.accounts.map((account) => (
                <AccountCard
                  key={accountKey(account)}
                  account={account}
                  selected={selectedKey === accountKey(account)}
                  busy={busy}
                  browsers={state.browsers}
                  onSelect={() =>
                    setSelectedKey((current) =>
                      current === accountKey(account) ? null : accountKey(account),
                    )
                  }
                  onSyncRename={() => confirmSyncRename(account)}
                  onReauthenticate={() => runCommand("reauthenticate_account", target(account))}
                  onSetBrowser={(browserId) =>
                    runCommand("set_account_browser", { ...target(account), browserId })
                  }
                  onOpenInBrowser={() => runCommand("open_account_in_browser", target(account))}
                />
              ))
            )}
          </div>
        </section>

        {login ? (
          <section className="authorization-widget" aria-label="授权进度">
            <div className="authorization-title">{loginTitle(login)}</div>
            <p className="authorization-description">
              {login.code
                ? `授权页已在 ${login.browserLabel} 打开，输入下方一次性授权码即可。应用不会读取或保存你的 Token。`
                : "正在向 GitHub 申请一次性授权码…"}
            </p>
            {login.code ? (
              <>
                <div className="auth-row">
                  <label htmlFor="authorizationCode">授权码</label>
                  <input
                    id="authorizationCode"
                    readOnly
                    value={login.code}
                    className="authorization-code"
                  />
                  <button type="button" onClick={() => copyText(login.code, "授权码已复制")}>
                    复制授权码
                  </button>
                </div>
                <div className="auth-row">
                  <label htmlFor="authorizationUrl">授权网址</label>
                  <input id="authorizationUrl" readOnly value={login.url} />
                  <div className="auth-row-actions">
                    <button type="button" onClick={() => copyText(login.url, "授权网址已复制")}>
                      复制网址
                    </button>
                    <button type="button" onClick={() => runCommand("reopen_authorization_page")}>
                      重新打开
                    </button>
                  </div>
                </div>
              </>
            ) : null}
            <div className="authorization-actions">
              <button type="button" className="danger" onClick={() => runCommand("cancel_login")}>
                取消登录
              </button>
            </div>
          </section>
        ) : null}

        <section className="bottom-actions">
          <button
            type="button"
            disabled={!switchEnabled}
            onClick={() => selectedAccount && runCommand("switch_account", target(selectedAccount))}
          >
            切换到所选账号
          </button>
          <button
            type="button"
            className="danger"
            disabled={!logoutEnabled}
            onClick={() => selectedAccount && confirmLogout(selectedAccount)}
          >
            登出所选账号
          </button>
          <div className="spacer" />
          <button
            type="button"
            className="primary"
            disabled={busy}
            onClick={() => setLoginDialogOpen(true)}
          >
            登录新账号
          </button>
        </section>
      </main>

      <footer className="status-bar">
        <span className={`status-dot ${state.message.isError ? "error" : ""}`}>●</span>
        <span
          className={`status-label ${state.message.isError ? "error" : ""}`}
          title={state.message.text}
        >
          {state.message.text}
        </span>
        <span className="status-version">GitHub CLI</span>
      </footer>

      {settingsOpen ? (
        <SettingsDialog
          initialPath={state.ghPath}
          onClose={() => setSettingsOpen(false)}
          onOpenReleases={() => void openGithubCliReleases()}
        />
      ) : null}

      {loginDialogOpen ? (
        <LoginDialog
          browsers={state.browsers}
          initialBrowser={state.lastLoginBrowser}
          onRescan={() => runCommand("rescan_browsers")}
          onCancel={() => setLoginDialogOpen(false)}
          onStart={(browserId) => {
            setLoginDialogOpen(false);
            void runCommand("login_account", { browserId });
          }}
        />
      ) : null}
    </div>
  );
}
