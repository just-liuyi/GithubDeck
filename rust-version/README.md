# GithubDeck (Rust + Tauri)

GithubDeck 是一款面向 Windows 的 GitHub CLI 多账号桌面管理器。本目录是与 Python 版功能等价的 **Rust + Tauri 2 + React** 实现：读取本机已安装的 GitHub CLI，并以图形界面完成状态刷新、账号切换、网页登录和本地登出。

统一项目说明和 Python/PySide6 实现请参阅仓库根目录的 [`README.md`](../README.md)。

## 功能

- 按 GitHub 主机显示本机 GitHub CLI 已保存的账号。
- 标识当前活动账号及需要重新认证的账号。
- 异步刷新认证状态，不阻塞界面。
- 切换到已登录的其他账号，并刷新确认真实状态。
- 通过 GitHub CLI 和系统默认浏览器添加账号。
- 显示一次性设备授权码，并仅在用户点击后复制网址或授权码。
- 取消正在等待的登录流程。
- 在二次确认后精确登出所选账号。
- 配置并验证自定义 `gh.exe` 路径。

## 技术栈

- Rust / Tauri 2 后端
- React 19 + TypeScript + Vite 前端
- 通过 `invoke` / 事件与本机 `gh.exe` 协作

## 系统要求

- Windows 10 或更高版本
- [GitHub CLI](https://github.com/cli/cli/releases/)
- 开发时需要：Node.js 18+、Rust stable、Visual Studio Build Tools（MSVC）

GithubDeck 默认查找：

```text
C:\Program Files\GitHub CLI\gh.exe
```

本项目测试使用的安装包是 `gh_2.96.0_windows_amd64.msi`。其他能够通过 `gh.exe --version` 校验的 GitHub CLI 版本也可以使用。

## 从源码运行

```powershell
cd rust-version
npm install
npm run tauri dev
```

生产构建：

```powershell
npm run tauri build
```

主要构建产物：

- `GithubDeck.exe`
- `GithubDeck_0.2.0_x64_en-US.msi`
- `GithubDeck_0.2.0_x64-setup.exe`

发布到 `v0.2.0` Release 时分别命名为：

- `GithubDeck-Rust-Portable-v0.2.0.exe`
- `GithubDeck-Rust-v0.2.0-x64.msi`
- `GithubDeck-Rust-v0.2.0-x64-setup.exe`

文件校验值与 Python 便携版一起收录在 `SHA256SUMS.txt`。

## 测试

后端单元测试：

```powershell
cd src-tauri
cargo test
```

## 使用方法

启动后，GithubDeck 会刷新本机 GitHub CLI 保存的账号：

- 选择非活动账号后，点击“切换到所选账号”。
- 点击“登录新账号”，在官方设备授权页输入界面显示的一次性授权码。
- 选择任意账号后，点击“登出所选账号”，并在确认窗口中明确确认。
- 点击“GitHub CLI 设置”可以选择其他 `gh.exe`；保存前会异步验证文件和版本。

登出执行精确的本地命令：

```text
gh auth logout --hostname HOST --user LOGIN
```

这只会移除本机 GitHub CLI 保存的认证配置，不会撤销 GitHub 端的 OAuth 授权。

## 安全边界

GithubDeck 不自行管理 GitHub 凭据：

- 不读取、保存或展示访问令牌。
- 不读取 Windows Credential Manager。
- 不调用 `gh auth token` 或 `--show-token`。
- 不把 GitHub CLI 的原始标准输出、错误输出、URL 或令牌展示到界面或写入日志。
- 登录期间仅以严格格式提取一次性设备授权短码。
- 授权码会在登录结束、失败或取消后立即清除。
- URL 和授权码只会在用户点击对应复制按钮后写入剪贴板。
- 每次启动 GitHub CLI 子进程都会移除继承的 GitHub Token 环境变量及 `GH_BROWSER`。
- 所有命令均通过程序路径和参数数组执行，不使用 shell 字符串拼接。

## 当前限制

- 不自动下载、安装或升级 GitHub CLI。
- 不管理或粘贴个人访问令牌。
- 不撤销 GitHub 端 OAuth 授权。
- 不将账号永久绑定到特定 Git 仓库。
- 不管理 `git push` 使用的 HTTPS 凭据或 SSH 密钥。

## 与 Python 版的关系

仓库中的 `python-version` 是 PySide6 实现；本目录是功能对等的 Rust/Tauri 重写，UI 文案与 GitHub 专业控制台视觉风格保持一致。
