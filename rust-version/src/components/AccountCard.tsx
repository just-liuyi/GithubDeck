import { AUTH_STATE_LABELS, type Account, type BrowserOption, displayName } from "../types";
import { highRiskScopes, scopeInfo, sortScopes } from "../scopes";

type Props = {
  account: Account;
  selected: boolean;
  busy: boolean;
  browsers: BrowserOption[];
  onSelect: () => void;
  onSyncRename: () => void;
  onReauthenticate: () => void;
  onSetBrowser: (browserId: string | null) => void;
  onOpenInBrowser: () => void;
};

function initials(login: string): string {
  const parts = login.replace(/[-_]/g, " ").split(/\s+/).filter(Boolean);
  if (parts.length > 1) {
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }
  return login.slice(0, 2).toUpperCase();
}

export default function AccountCard({
  account,
  selected,
  busy,
  browsers,
  onSelect,
  onSyncRename,
  onReauthenticate,
  onSetBrowser,
  onOpenInBrowser,
}: Props) {
  const name = displayName(account);
  const risky = highRiskScopes(account.scopes);
  const needsLogin = account.state === "failure";
  const badgeClass = account.active ? "active" : needsLogin ? "needs-login" : "";
  const browserMissing = account.browser !== null && !account.browserAvailable;

  return (
    <div
      className={`account-card ${selected ? "selected" : ""} ${account.active ? "active" : ""}`}
      role="listitem"
      aria-label={name}
    >
      <button
        type="button"
        className="account-main"
        aria-pressed={selected}
        disabled={busy}
        onClick={onSelect}
      >
        <div className={`account-accent ${account.active ? "active" : ""}`} />
        <div className="account-avatar">{initials(name)}</div>
        <div className="account-copy">
          <div className="account-login">
            {name}
            {account.renamed ? (
              <span className="renamed-tag" title="GitHub CLI 仍以旧名称保存此账号">
                本机记录：{account.login}
              </span>
            ) : null}
          </div>
          <div className="account-meta">
            {account.host} · {AUTH_STATE_LABELS[account.state]}
            {account.browser ? (
              <span className={browserMissing ? "meta-warning" : ""}>
                {" "}
                · {account.browser.label}
                {browserMissing ? "（未找到）" : ""}
              </span>
            ) : null}
          </div>
        </div>
        <div className="account-tags">
          {risky.length > 0 ? (
            <span className="account-badge risk" title={`高风险权限：${risky.join("、")}`}>
              高风险权限
            </span>
          ) : null}
          <span className={`account-badge ${badgeClass}`}>
            {account.active ? "当前账号" : AUTH_STATE_LABELS[account.state]}
          </span>
        </div>
      </button>

      {selected ? (
        <div className="account-details">
          {account.renamed ? (
            <div className="notice warning">
              <span>
                此账号已在 GitHub 改名为 <b>{account.githubLogin}</b>，GitHub CLI 仍以{" "}
                <b>{account.login}</b> 保存。
              </span>
              <button type="button" disabled={busy} onClick={onSyncRename}>
                同步改名
              </button>
            </div>
          ) : null}
          {needsLogin ? (
            <div className="notice error">
              <span>此账号的凭据已失效，需要重新认证。</span>
              <button type="button" disabled={busy} onClick={onReauthenticate}>
                重新认证
              </button>
            </div>
          ) : null}

          <div className="detail-row">
            <span className="detail-label">权限</span>
            <div className="detail-body">
              {account.scopes.length > 0 ? (
                <div className="scope-list">
                  {sortScopes(account.scopes).map((scope) => {
                    const info = scopeInfo(scope);
                    return (
                      <span
                        key={scope}
                        className={`scope-chip ${info.risk}`}
                        title={info.description}
                      >
                        {scope}
                      </span>
                    );
                  })}
                </div>
              ) : (
                <span className="detail-muted">
                  {needsLogin
                    ? "凭据失效，无法读取权限"
                    : "未报告经典权限（可能是细粒度令牌或 GitHub App 令牌）"}
                </span>
              )}
              {risky.length > 0 ? (
                <p className="scope-warning">
                  该令牌可以执行危险操作：
                  {risky.map((scope) => `${scope}（${scopeInfo(scope).description}）`).join("、")}
                  。不需要时，可在终端切换到此账号后执行{" "}
                  <code>
                    gh auth refresh -h {account.host} --remove-scopes {risky.join(",")}
                  </code>{" "}
                  移除。
                </p>
              ) : null}
            </div>
          </div>

          <div className="detail-row">
            <span className="detail-label">浏览器</span>
            <div className="detail-body browser-row">
              <select
                aria-label="绑定浏览器"
                value={account.browser?.id ?? ""}
                disabled={busy}
                onChange={(event) => onSetBrowser(event.target.value || null)}
              >
                <option value="">未绑定（使用系统默认浏览器）</option>
                {browserMissing && account.browser ? (
                  <option value={account.browser.id} disabled>
                    {account.browser.label}（未找到）
                  </option>
                ) : null}
                {browsers.map((browser) => (
                  <option key={browser.id} value={browser.id}>
                    {browser.label}
                  </option>
                ))}
              </select>
              <button type="button" disabled={browserMissing} onClick={onOpenInBrowser}>
                在此浏览器打开 GitHub
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
