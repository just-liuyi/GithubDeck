import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { Account, AppState } from "./types";

const { invokeMock, listenMock, openUrlMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  openUrlMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: openUrlMock }));

const RELEASES_URL = "https://github.com/cli/cli/releases/";

function account(overrides: Partial<Account>): Account {
  return {
    host: "github.com",
    login: "alice",
    githubLogin: "alice",
    renamed: false,
    active: false,
    state: "success",
    scopes: ["gist", "read:org", "repo"],
    browser: null,
    browserAvailable: false,
    ...overrides,
  };
}

const chrome = { id: "Google Chrome|Default", label: "Google Chrome · Personal" };
const edge = { id: "Microsoft Edge|Default", label: "Microsoft Edge · Work" };

function baseState(overrides: Partial<AppState> = {}): AppState {
  return {
    accounts: [],
    busy: false,
    login: null,
    message: { text: "准备就绪", isError: false },
    ghPath: "C:\\Program Files\\GitHub CLI\\gh.exe",
    browsers: [chrome, edge],
    lastLoginBrowser: null,
    ...overrides,
  };
}

let currentState: AppState;
let pushState: ((state: AppState) => void) | undefined;

beforeEach(() => {
  currentState = baseState();
  pushState = undefined;
  invokeMock.mockReset();
  listenMock.mockReset();
  openUrlMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === "get_state") return Promise.resolve(currentState);
    return Promise.resolve(null);
  });
  listenMock.mockImplementation((_event: string, handler: (event: { payload: AppState }) => void) => {
    pushState = (state) => handler({ payload: state });
    return Promise.resolve(() => undefined);
  });
  openUrlMock.mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function commandCalls(command: string) {
  return invokeMock.mock.calls.filter(([name]) => name === command);
}

async function renderWith(state: AppState) {
  currentState = state;
  render(<App />);
  await waitFor(() => expect(commandCalls("get_state").length).toBe(1));
}

async function openSettings() {
  await renderWith(baseState());
  fireEvent.click(screen.getByRole("button", { name: "GitHub CLI 设置" }));
  return screen.findByRole("dialog");
}

describe("GithubDeck branding and official releases link", () => {
  it("shows the GithubDeck brand and fixed official releases URL", async () => {
    await openSettings();

    expect(screen.getByText("GithubDeck")).toBeTruthy();
    const link = screen.getByRole("link", { name: `官方发布页：${RELEASES_URL}` });
    expect(link.getAttribute("href")).toBe(RELEASES_URL);
    expect(screen.queryByText(/测试安装包/)).toBeNull();
  });

  it("prevents webview navigation and opens only the fixed URL", async () => {
    await openSettings();
    const link = screen.getByRole("link", { name: /官方发布页/ });
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });

    link.dispatchEvent(click);

    expect(click.defaultPrevented).toBe(true);
    await waitFor(() => {
      expect(openUrlMock).toHaveBeenCalledTimes(1);
      expect(openUrlMock).toHaveBeenCalledWith(RELEASES_URL);
    });
  });

  it("keeps settings open and reports the fixed error when opener rejects", async () => {
    openUrlMock.mockRejectedValueOnce(new Error("native opener details"));
    await openSettings();

    fireEvent.click(screen.getByRole("link", { name: /官方发布页/ }));

    expect(await screen.findByRole("dialog")).toBeTruthy();
    await waitFor(() => {
      const status = screen.getByText("无法打开 GitHub CLI 官方发布页，请稍后重试");
      expect(status.classList.contains("error")).toBe(true);
    });
    expect(screen.queryByText("native opener details")).toBeNull();
  });
});

describe("GitHub CLI settings", () => {
  it("fills the path from the native picker and saves with one backend call", async () => {
    await openSettings();
    invokeMock.mockImplementation((command: string) => {
      if (command === "pick_gh_executable") return Promise.resolve("D:\\tools\\gh.exe");
      if (command === "save_gh_path") return Promise.resolve("D:\\tools\\gh.exe");
      return Promise.resolve(null);
    });

    fireEvent.click(screen.getByRole("button", { name: "选择 gh.exe" }));
    const input = screen.getByLabelText("当前路径") as HTMLInputElement;
    await waitFor(() => expect(input.value).toBe("D:\\tools\\gh.exe"));

    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(commandCalls("save_gh_path")).toEqual([["save_gh_path", { path: "D:\\tools\\gh.exe" }]]);
    expect(commandCalls("validate_gh_path")).toEqual([]);
  });

  it("shows the validation error and stays open", async () => {
    await openSettings();
    invokeMock.mockImplementation((command: string) =>
      command === "save_gh_path" ? Promise.reject("请选择名为 gh.exe 的文件") : Promise.resolve(null),
    );
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("请选择名为 gh.exe 的文件")).toBeTruthy();
    expect(screen.getByRole("dialog")).toBeTruthy();
  });
});

describe("account cards", () => {
  it("flags high-risk token permissions without selecting the account", async () => {
    await renderWith(
      baseState({ accounts: [account({ active: true, scopes: ["delete_repo", "gist", "repo"] })] }),
    );
    const card = await screen.findByRole("listitem", { name: "alice" });
    expect(within(card).getByText("高风险权限")).toBeTruthy();

    fireEvent.click(within(card).getByRole("button", { name: /alice/ }));
    const chip = within(card).getByText("delete_repo");
    expect(chip.classList.contains("high")).toBe(true);
    expect(chip.getAttribute("title")).toBe("删除仓库");
    expect(within(card).getByText(/--remove-scopes delete_repo/)).toBeTruthy();
  });

  it("shows the current GitHub name of a renamed account and syncs after confirmation", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    await renderWith(
      baseState({
        accounts: [account({ login: "alice", githubLogin: "alice-new", renamed: true, browser: chrome, browserAvailable: true })],
      }),
    );
    const card = await screen.findByRole("listitem", { name: "alice-new" });
    expect(within(card).getByText("本机记录：alice")).toBeTruthy();

    fireEvent.click(within(card).getByRole("button", { name: /alice-new/ }));
    fireEvent.click(within(card).getByRole("button", { name: "同步改名" }));

    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining("Google Chrome · Personal"));
    await waitFor(() =>
      expect(commandCalls("sync_account_rename")).toEqual([
        ["sync_account_rename", { host: "github.com", login: "alice" }],
      ]),
    );
  });

  it("offers re-authentication for accounts whose token failed", async () => {
    await renderWith(baseState({ accounts: [account({ login: "bob", githubLogin: null, state: "failure", scopes: [] })] }));
    const card = await screen.findByRole("listitem", { name: "bob" });
    fireEvent.click(within(card).getByRole("button", { name: /bob/ }));
    expect(within(card).getByText("凭据失效，无法读取权限")).toBeTruthy();
    fireEvent.click(within(card).getByRole("button", { name: "重新认证" }));
    await waitFor(() =>
      expect(commandCalls("reauthenticate_account")).toEqual([
        ["reauthenticate_account", { host: "github.com", login: "bob" }],
      ]),
    );
  });

  it("binds a browser and opens GitHub in it", async () => {
    await renderWith(baseState({ accounts: [account({ browser: chrome, browserAvailable: true })] }));
    const card = await screen.findByRole("listitem", { name: "alice" });
    expect(within(card).getByText(/Google Chrome · Personal/)).toBeTruthy();
    fireEvent.click(within(card).getByRole("button", { name: /alice/ }));

    fireEvent.change(within(card).getByLabelText("绑定浏览器"), { target: { value: edge.id } });
    await waitFor(() =>
      expect(commandCalls("set_account_browser")).toEqual([
        ["set_account_browser", { host: "github.com", login: "alice", browserId: edge.id }],
      ]),
    );
    fireEvent.change(within(card).getByLabelText("绑定浏览器"), { target: { value: "" } });
    await waitFor(() => expect(commandCalls("set_account_browser")[1][1].browserId).toBeNull());

    fireEvent.click(within(card).getByRole("button", { name: "在此浏览器打开 GitHub" }));
    await waitFor(() => expect(commandCalls("open_account_in_browser").length).toBe(1));
  });

  it("marks a bound browser that is no longer installed", async () => {
    await renderWith(baseState({ accounts: [account({ browser: { id: "Gone|", label: "Old Browser" }, browserAvailable: false })] }));
    const card = await screen.findByRole("listitem", { name: "alice" });
    expect(within(card).getByText(/Old Browser（未找到）/)).toBeTruthy();
    fireEvent.click(within(card).getByRole("button", { name: /alice/ }));
    expect(
      (within(card).getByRole("button", { name: "在此浏览器打开 GitHub" }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});

describe("login", () => {
  it("asks which browser to authorize in, preselecting the last choice", async () => {
    await renderWith(baseState({ lastLoginBrowser: edge.id }));
    fireEvent.click(screen.getByRole("button", { name: "登录新账号" }));
    const dialog = await screen.findByRole("dialog");
    expect((within(dialog).getByLabelText(edge.label) as HTMLInputElement).checked).toBe(true);

    fireEvent.click(within(dialog).getByLabelText(chrome.label));
    fireEvent.click(within(dialog).getByRole("button", { name: "开始登录" }));

    await waitFor(() =>
      expect(commandCalls("login_account")).toEqual([["login_account", { browserId: chrome.id }]]),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("sends null for the system default browser", async () => {
    await renderWith(baseState());
    fireEvent.click(screen.getByRole("button", { name: "登录新账号" }));
    fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "开始登录" }));
    await waitFor(() =>
      expect(commandCalls("login_account")).toEqual([["login_account", { browserId: null }]]),
    );
  });

  it("shows the device code with the browser used, and can cancel", async () => {
    await renderWith(baseState());
    pushState?.(
      baseState({
        busy: true,
        login: {
          purpose: "new",
          targetLogin: null,
          code: "ABCD-1234",
          url: "https://github.com/login/device",
          browserLabel: chrome.label,
        },
      }),
    );
    const widget = await screen.findByRole("region", { name: "授权进度" });
    expect((within(widget).getByLabelText("授权码") as HTMLInputElement).value).toBe("ABCD-1234");
    expect(within(widget).getByText(/授权页已在 Google Chrome · Personal 打开/)).toBeTruthy();

    fireEvent.click(within(widget).getByRole("button", { name: "重新打开" }));
    fireEvent.click(within(widget).getByRole("button", { name: "取消登录" }));
    await waitFor(() => {
      expect(commandCalls("reopen_authorization_page").length).toBe(1);
      expect(commandCalls("cancel_login").length).toBe(1);
    });
  });
});
