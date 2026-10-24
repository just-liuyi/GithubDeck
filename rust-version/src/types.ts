export type AuthState = "success" | "failure" | "timeout" | "unknown";

export type BrowserOption = {
  id: string;
  label: string;
};

export type Account = {
  host: string;
  /** Name GitHub CLI stored the account under. */
  login: string;
  /** Name GitHub reports now, when known. */
  githubLogin: string | null;
  renamed: boolean;
  active: boolean;
  state: AuthState;
  scopes: string[];
  browser: BrowserOption | null;
  browserAvailable: boolean;
};

export type LoginPurpose = "new" | "reauth" | "rename";

export type LoginView = {
  purpose: LoginPurpose;
  targetLogin: string | null;
  code: string;
  url: string;
  browserLabel: string;
};

export type UiMessage = {
  text: string;
  isError: boolean;
};

export type AppState = {
  accounts: Account[];
  busy: boolean;
  login: LoginView | null;
  message: UiMessage;
  ghPath: string;
  browsers: BrowserOption[];
  lastLoginBrowser: string | null;
};

export const AUTH_STATE_LABELS: Record<AuthState, string> = {
  success: "已认证",
  failure: "需重新认证",
  timeout: "状态检查超时",
  unknown: "认证状态未知",
};

export function accountKey(account: Pick<Account, "host" | "login">): string {
  return `${account.host}|${account.login}`;
}

export function displayName(account: Account): string {
  return account.githubLogin ?? account.login;
}
