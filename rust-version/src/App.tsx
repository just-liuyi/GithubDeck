import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

type AuthState = "success" | "failure" | "unknown";

type Account = {
  host: string;
  login: string;
  active: boolean;
  state: AuthState;
};

type UiMessage = {
  text: string;
  isError: boolean;
};

type AppState = {
  accounts: Account[];
  busy: boolean;
  loginActive: boolean;
  authorizationCode: string;
  message: UiMessage;
  ghPath: string;
};

const DEVICE_LOGIN_URL = "https://github.com/login/device";
const GITHUB_CLI_RELEASES_URL = "https://github.com/cli/cli/releases/";
const AUTH_STATE_LABELS: Record<AuthState, string> = {
  success: "已认证",
  failure: "需重新认证",
  unknown: "认证状态未知",
};

const emptyState: AppState = {
  accounts: [],
  busy: false,
  loginActive: false,
  authorizationCode: "",
  message: { text: "准备就绪", isError: false },
  ghPath: "",
};

function initials(login: string): string {
  const parts = login.replace(/[-_]/g, " ").split(/\s+/).filter(Boolean);
  if (parts.length > 1) {
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
  return login.slice(0, 2).toUpperCase();
}

export default function App() {
  const [state, setState] = useState<AppState>(emptyState);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsPath, setSettingsPath] = useState("");
  const [settingsStatus, setSettingsStatus] = useState("选择 gh.exe 后可校验并保存");
  const [settingsState, setSettingsState] = useState<"idle" | "validating" | "success" | "error">(
    "idle",
  );
  const [settingsBusy, setSettingsBusy] = useState(false);

  const selectedAccount = useMemo(() => {
    if (!selectedKey) return null;
    return (
      state.accounts.find((account) => `${account.host}|${account.login}` === selectedKey) ?? null
    );
  }, [selectedKey, state.accounts]);

  const applyState = useCallback((next: AppState) => {
    setState(next);
    setSelectedKey((current) => {
      if (!current) return current;
      const stillThere = next.accounts.some(
        (account) => `${account.host}|${account.login}` === current,
      );
      return stillThere ? current : null;
    });
  }, []);

  useEffect(() => {
    let unlistenState: (() => void) | undefined;
    let unlistenCode: (() => void) | undefined;

    (async () => {
      try {
        const initial = await invoke<AppState>("get_state");
        applyState(initial);
        setSettingsPath(initial.ghPath);
      } catch {
        // ignore bootstrap errors; events will update later
      }

      unlistenState = await listen<AppState>("app-state", (event) => {
        applyState(event.payload);
        setSettingsPath(event.payload.ghPath);
      });
      unlistenCode = await listen<{ code: string }>("authorization-code", (event) => {
        setState((current) => ({
          ...current,
          authorizationCode: event.payload.code,
          loginActive: true,
        }));
      });
    })();

    return () => {
      unlistenState?.();
      unlistenCode?.();
    };
  }, [applyState]);

  async function runCommand(command: string, args?: Record<string, unknown>) {
    try {
      await invoke(command, args);
    } catch (error) {
      const text = typeof error === "string" ? error : "操作失败";
      setState((current) => ({
        ...current,
        message: { text, isError: true },
      }));
    }
  }

  async function copyText(text: string, successMessage: string) {
    try {
      await navigator.clipboard.writeText(text);
      setState((current) => ({
        ...current,
        message: { text: successMessage, isError: false },
      }));
    } catch {
      setState((current) => ({
        ...current,
        message: { text: "无法访问系统剪贴板，请稍后重试", isError: true },
      }));
    }
  }

  async function confirmLogout(account: Account) {
    const ok = window.confirm(
      `确定要从本机 GitHub CLI 登出 ${account.login}（${account.host}）吗？\n\n这只会移除本机保存的认证配置，不会撤销 GitHub 端的 OAuth 授权。`,
    );
    if (!ok) return;
    await runCommand("logout_account", { host: account.host, login: account.login });
  }

  async function openSettings() {
    try {
      const path = await invoke<string>("get_gh_path");
      setSettingsPath(path);
    } catch {
      // keep current path
    }
    setSettingsStatus("选择 gh.exe 后可校验并保存");
    setSettingsState("idle");
    setSettingsOpen(true);
  }

  async function validateAndSave(path: string) {
    setSettingsBusy(true);
    setSettingsState("validating");
    setSettingsStatus("正在校验 GitHub CLI…");
    try {
      const result = await invoke<{ valid: boolean; message: string }>("validate_gh_path", {
        path,
      });
      if (!result.valid) {
        setSettingsState("error");
        setSettingsStatus(result.message);
        return;
      }
      const saved = await invoke<string>("save_gh_path", { path });
      setSettingsPath(saved);
      setSettingsState("success");
      setSettingsStatus("GitHub CLI 校验通过，路径已保存");
      setSettingsOpen(false);
    } catch (error) {
      setSettingsState("error");
      setSettingsStatus(typeof error === "string" ? error : "保存失败");
    } finally {
      setSettingsBusy(false);
    }
  }

  async function openGithubCliReleases() {
    try {
      await openUrl(GITHUB_CLI_RELEASES_URL);
    } catch {
      setState((current) => ({
        ...current,
        message: {
          text: "无法打开 GitHub CLI 官方发布页，请稍后重试",
          isError: true,
        },
      }));
    }
  }

  async function restoreDefault() {
    setSettingsBusy(true);
    setSettingsState("validating");
    setSettingsStatus("正在恢复默认路径…");
    try {
      const path = await invoke<string>("reset_gh_path");
      setSettingsPath(path);
      setSettingsState("success");
      setSettingsStatus("已恢复默认 GitHub CLI 路径");
      setSettingsOpen(false);
    } catch (error) {
      setSettingsState("error");
      setSettingsStatus(typeof error === "string" ? error : "恢复默认失败");
    } finally {
      setSettingsBusy(false);
    }
  }

  function onPickGhExe() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".exe";
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return;
      const anyFile = file as File & { path?: string };
      const candidate = anyFile.path || file.name;
      setSettingsPath(candidate);
    };
    input.click();
  }

  const switchEnabled = !state.busy && selectedAccount !== null && !selectedAccount.active;
  const logoutEnabled = !state.busy && selectedAccount !== null;

  return (
    <div className="main-container" id="mainContainer">
      <header className="app-header" id="appHeader">
        <img className="app-mark" id="appMark" src="/favicon.png" alt="" />
        <div className="app-title" id="appTitle">
          GithubDeck
        </div>
      </header>

      <main className="content">
        <section className="page-header" id="pageHeader">
          <div>
            <h1 className="page-title" id="pageTitle">
              GitHub 账号
            </h1>
            <p className="page-description" id="pageDescription">
              管理本机 GitHub CLI 中已认证的账号
            </p>
          </div>
          <div className="page-actions">
            <button
              id="refreshButton"
              disabled={state.busy}
              onClick={() => runCommand("refresh_accounts")}
            >
              刷新
            </button>
            <button
              id="settingsButton"
              disabled={state.busy}
              onClick={openSettings}
              title="GitHub CLI 设置"
            >
              GitHub CLI 设置
            </button>
          </div>
        </section>

        <section className="account-section" id="accountSection">
          <div className="account-heading">
            <span className="account-section-label" id="accountSectionLabel">
              已认证账号
            </span>
            <span className="account-count-label" id="accountCountLabel">
              {state.accounts.length} 个账号
            </span>
          </div>
          <div className={`account-list ${state.busy ? "disabled" : ""}`} id="accountList">
            {state.accounts.length === 0 ? (
              <div className="empty-accounts">暂无账号，可点击“登录新账号”添加</div>
            ) : (
              state.accounts.map((account) => {
                const key = `${account.host}|${account.login}`;
                const selected = selectedKey === key;
                return (
                  <button
                    type="button"
                    key={key}
                    className={`account-card ${selected ? "selected" : ""} ${
                      account.active ? "active" : ""
                    }`}
                    disabled={state.busy}
                    onClick={() => setSelectedKey(key)}
                  >
                    <div
                      className={`account-accent ${account.active ? "active" : ""}`}
                      id="accountAccent"
                    />
                    <div className="account-avatar" id="accountAvatar">
                      {initials(account.login)}
                    </div>
                    <div className="account-copy">
                      <div className="account-login" id="accountLogin">
                        {account.login}
                      </div>
                      <div className="account-meta" id="accountMeta">
                        {account.host} · {AUTH_STATE_LABELS[account.state]}
                      </div>
                    </div>
                    <div
                      className={`account-badge ${account.active ? "active" : ""} ${
                        account.state === "failure" ? "needs-login" : ""
                      }`}
                      id="accountBadge"
                    >
                      {account.active ? "当前账号" : AUTH_STATE_LABELS[account.state]}
                    </div>
                  </button>
                );
              })
            )}
          </div>
        </section>

        {state.loginActive && state.authorizationCode ? (
          <section className="authorization-widget" id="authorizationWidget">
            <div className="authorization-title" id="authorizationTitle">
              在浏览器中完成 GitHub 授权
            </div>
            <p className="authorization-description" id="authorizationDescription">
              打开官方设备授权页并输入一次性授权码。应用不会读取或保存你的 Token。
            </p>
            <div className="auth-row">
              <label>授权网址</label>
              <input
                id="authorizationUrl"
                readOnly
                value={DEVICE_LOGIN_URL}
                className="authorization-url"
              />
              <button
                id="copyUrlButton"
                onClick={() => copyText(DEVICE_LOGIN_URL, "授权网址已复制")}
              >
                复制网址
              </button>
            </div>
            <div className="auth-row">
              <label>授权码</label>
              <input
                id="authorizationCode"
                readOnly
                value={state.authorizationCode}
                className="authorization-code"
              />
              <button
                id="copyCodeButton"
                onClick={() => copyText(state.authorizationCode, "授权码已复制")}
              >
                复制授权码
              </button>
            </div>
          </section>
        ) : null}

        <section className="bottom-actions" id="bottomActions">
          <button
            id="switchButton"
            disabled={!switchEnabled}
            onClick={() => {
              if (!selectedAccount) return;
              runCommand("switch_account", {
                host: selectedAccount.host,
                login: selectedAccount.login,
              });
            }}
          >
            切换到所选账号
          </button>
          <button
            id="logoutButton"
            className="danger"
            disabled={!logoutEnabled}
            onClick={() => {
              if (!selectedAccount) return;
              confirmLogout(selectedAccount);
            }}
          >
            登出所选账号
          </button>
          <div className="spacer" />
          {state.loginActive ? (
            <button
              id="cancelButton"
              className="danger"
              onClick={() => runCommand("cancel_login")}
            >
              取消登录
            </button>
          ) : null}
          <button
            id="loginButton"
            className="primary"
            disabled={state.busy}
            onClick={() => runCommand("login_account")}
          >
            登录新账号
          </button>
        </section>
      </main>

      <footer className="status-bar" id="statusBar">
        <span
          className={`status-dot ${state.message.isError ? "error" : ""}`}
          id="statusDot"
        >
          ●
        </span>
        <span
          className={`status-label ${state.message.isError ? "error" : ""}`}
          id="statusLabel"
        >
          {state.message.text}
        </span>
        <span className="status-version" id="statusVersion">
          GitHub CLI
        </span>
      </footer>

      {settingsOpen ? (
        <div className="dialog-backdrop" role="dialog" aria-modal="true">
          <div className="dialog-container" id="dialogContainer">
            <div className="dialog-header" id="ghSettingsHeader">
              <h2 className="dialog-title" id="dialogTitle">
                GitHub CLI 设置
              </h2>
              <p className="dialog-description" id="dialogDescription">
                指定本机 gh.exe 路径。保存前会校验文件名与版本输出。
              </p>
            </div>
            <div className="settings-card" id="ghSettingsPathCard">
              <label>当前路径</label>
              <input
                value={settingsPath}
                onChange={(event) => setSettingsPath(event.target.value)}
                spellCheck={false}
              />
              <div className="settings-actions">
                <button disabled={settingsBusy} onClick={onPickGhExe}>
                  选择 gh.exe
                </button>
                <button disabled={settingsBusy} onClick={restoreDefault}>
                  恢复默认
                </button>
              </div>
              <div className={`settings-status ${settingsState}`} id="ghSettingsStatus">
                {settingsStatus}
              </div>
            </div>
            <div className="settings-card" id="ghSettingsHelpCard">
              <a
                className="source-label source-link"
                id="ghSourceLabel"
                href={GITHUB_CLI_RELEASES_URL}
                onClick={(event) => {
                  event.preventDefault();
                  void openGithubCliReleases();
                }}
              >
                官方发布页：{GITHUB_CLI_RELEASES_URL}
              </a>
              <div className="source-label">测试安装包：gh_2.96.0_windows_amd64.msi</div>
            </div>
            <div className="dialog-actions">
              <button disabled={settingsBusy} onClick={() => setSettingsOpen(false)}>
                取消
              </button>
              <button
                id="saveButton"
                className="primary"
                disabled={settingsBusy || !settingsPath.trim()}
                onClick={() => validateAndSave(settingsPath.trim())}
              >
                保存
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
