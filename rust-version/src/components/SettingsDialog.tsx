import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export const GITHUB_CLI_RELEASES_URL = "https://github.com/cli/cli/releases/";

type Status = "idle" | "validating" | "success" | "error";

type Props = {
  initialPath: string;
  onClose: () => void;
  onOpenReleases: () => void;
};

function errorText(error: unknown, fallback: string): string {
  return typeof error === "string" ? error : fallback;
}

export default function SettingsDialog({ initialPath, onClose, onOpenReleases }: Props) {
  const [path, setPath] = useState(initialPath);
  const [status, setStatus] = useState<Status>("idle");
  const [statusText, setStatusText] = useState("选择或输入 gh.exe 路径后保存，保存前会校验");
  const [busy, setBusy] = useState(false);

  async function run(progress: string, action: () => Promise<void>, fallback: string) {
    setBusy(true);
    setStatus("validating");
    setStatusText(progress);
    try {
      await action();
    } catch (error) {
      setStatus("error");
      setStatusText(errorText(error, fallback));
    } finally {
      setBusy(false);
    }
  }

  async function pick() {
    try {
      const picked = await invoke<string | null>("pick_gh_executable");
      if (picked) {
        setPath(picked);
        setStatus("idle");
        setStatusText("已选择文件，点击保存进行校验");
      }
    } catch (error) {
      setStatus("error");
      setStatusText(errorText(error, "无法打开文件选择对话框"));
    }
  }

  return (
    <div className="dialog-backdrop" role="dialog" aria-modal="true" aria-labelledby="settingsDialogTitle">
      <div className="dialog-container">
        <div className="dialog-header">
          <h2 className="dialog-title" id="settingsDialogTitle">
            GitHub CLI 设置
          </h2>
          <p className="dialog-description">指定本机 gh.exe 路径。保存前会校验文件名与版本输出。</p>
        </div>
        <div className="settings-card">
          <label htmlFor="ghPathInput">当前路径</label>
          <input
            id="ghPathInput"
            value={path}
            onChange={(event) => setPath(event.target.value)}
            spellCheck={false}
          />
          <div className="settings-actions">
            <button type="button" disabled={busy} onClick={pick}>
              选择 gh.exe
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                run(
                  "正在校验默认路径…",
                  async () => {
                    const saved = await invoke<string>("reset_gh_path");
                    setPath(saved);
                    onClose();
                  },
                  "恢复默认失败",
                )
              }
            >
              恢复默认
            </button>
          </div>
          <div className={`settings-status ${status}`} id="ghSettingsStatus">
            {statusText}
          </div>
        </div>
        <div className="settings-card">
          <a
            className="source-label source-link"
            href={GITHUB_CLI_RELEASES_URL}
            onClick={(event) => {
              event.preventDefault();
              onOpenReleases();
            }}
          >
            官方发布页：{GITHUB_CLI_RELEASES_URL}
          </a>
        </div>
        <div className="dialog-actions">
          <button type="button" disabled={busy} onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="primary"
            disabled={busy || !path.trim()}
            onClick={() =>
              run(
                "正在校验 GitHub CLI…",
                async () => {
                  await invoke<string>("save_gh_path", { path: path.trim() });
                  onClose();
                },
                "保存失败",
              )
            }
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
