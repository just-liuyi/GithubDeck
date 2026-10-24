import { useState } from "react";
import type { BrowserOption } from "../types";

type Props = {
  browsers: BrowserOption[];
  initialBrowser: string | null;
  onRescan: () => void;
  onCancel: () => void;
  onStart: (browserId: string | null) => void;
};

export default function LoginDialog({ browsers, initialBrowser, onRescan, onCancel, onStart }: Props) {
  const [choice, setChoice] = useState<string>(initialBrowser ?? "");

  return (
    <div className="dialog-backdrop" role="dialog" aria-modal="true" aria-labelledby="loginDialogTitle">
      <div className="dialog-container">
        <div className="dialog-header">
          <h2 className="dialog-title" id="loginDialogTitle">
            登录新账号
          </h2>
          <p className="dialog-description">
            选择要在哪个浏览器中完成授权。请确认该浏览器里登录的就是要添加的 GitHub 账号；登录成功后，新账号会自动绑定这个浏览器。
          </p>
        </div>
        <div className="settings-card">
          <div className="browser-choices" role="radiogroup" aria-label="授权浏览器">
            <label className="browser-choice">
              <input
                type="radio"
                name="loginBrowser"
                value=""
                checked={choice === ""}
                onChange={() => setChoice("")}
              />
              系统默认浏览器
            </label>
            {browsers.map((browser) => (
              <label className="browser-choice" key={browser.id}>
                <input
                  type="radio"
                  name="loginBrowser"
                  value={browser.id}
                  checked={choice === browser.id}
                  onChange={() => setChoice(browser.id)}
                />
                {browser.label}
              </label>
            ))}
          </div>
          <div className="settings-actions">
            <button type="button" onClick={onRescan}>
              重新扫描浏览器
            </button>
          </div>
        </div>
        <div className="dialog-actions">
          <button type="button" onClick={onCancel}>
            取消
          </button>
          <button
            type="button"
            className="primary"
            onClick={() => onStart(choice === "" ? null : choice)}
          >
            开始登录
          </button>
        </div>
      </div>
    </div>
  );
}
