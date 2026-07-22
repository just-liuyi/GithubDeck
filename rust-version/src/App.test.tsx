import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const { invokeMock, listenMock, openUrlMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  openUrlMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: openUrlMock }));

const initialState = {
  accounts: [],
  busy: false,
  loginActive: false,
  authorizationCode: "",
  message: { text: "准备就绪", isError: false },
  ghPath: "C:\\Program Files\\GitHub CLI\\gh.exe",
};

afterEach(cleanup);

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockReset();
  openUrlMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === "get_state") return Promise.resolve(initialState);
    if (command === "get_gh_path") return Promise.resolve(initialState.ghPath);
    return Promise.resolve();
  });
  listenMock.mockResolvedValue(() => undefined);
  openUrlMock.mockResolvedValue(undefined);
});

async function openSettings() {
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "GitHub CLI 设置" }));
  return screen.findByRole("dialog");
}

describe("GithubDeck branding and official releases link", () => {
  it("shows the GithubDeck brand and fixed official releases URL", async () => {
    await openSettings();

    expect(screen.getByText("GithubDeck")).toBeTruthy();
    const link = screen.getByRole("link", { name: `官方发布页：${"https://github.com/cli/cli/releases/"}` });
    expect(link.getAttribute("href")).toBe("https://github.com/cli/cli/releases/");
  });

  it("prevents webview navigation and opens only the fixed URL", async () => {
    await openSettings();
    const link = screen.getByRole("link", { name: /官方发布页/ });
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });

    link.dispatchEvent(click);

    expect(click.defaultPrevented).toBe(true);
    await waitFor(() => {
      expect(openUrlMock).toHaveBeenCalledTimes(1);
      expect(openUrlMock).toHaveBeenCalledWith("https://github.com/cli/cli/releases/");
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
