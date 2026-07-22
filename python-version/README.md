# GithubDeck

GithubDeck 是一款面向 Windows 的 GitHub CLI 多账号桌面管理器。它使用本机已经安装的 GitHub CLI 读取已认证账号，并以图形界面完成状态刷新、账号切换、网页登录和本地登出。

本目录是 Python 3.13 + PySide6 实现。统一项目说明和 Rust/Tauri 实现请参阅仓库根目录的 [`README.md`](../README.md)。

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

## 系统要求

- Windows 10 或更高版本
- [GitHub CLI](https://github.com/cli/cli/releases/)

GithubDeck 默认查找：

```text
C:\Program Files\GitHub CLI\gh.exe
```

本项目测试使用的安装包是 `gh_2.96.0_windows_amd64.msi`。其他能够通过 `gh.exe --version` 校验的 GitHub CLI 版本也可以使用。

## 下载与运行

1. 打开项目的 `v0.2.0` GitHub Release。
2. 下载 `GithubDeck-Python-Portable-v0.2.0.exe`。
3. 下载同一 Release 中的 `SHA256SUMS.txt` 并校验文件完整性。
4. 双击 EXE 启动。

校验 SHA-256：

```powershell
Get-FileHash .\GithubDeck-Python-Portable-v0.2.0.exe -Algorithm SHA256
```

该 EXE 未进行商业代码签名，Windows SmartScreen 可能显示未知发布者提示。请只从本项目的官方 GitHub Release 下载，并与 `SHA256SUMS.txt` 核对。

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

`gh auth switch` 修改的是指定 GitHub 主机的全局活动账号。其他终端随后执行的 `gh` 命令也会使用新账号。

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
- 首版 Windows EXE 未进行代码签名。

## 从源码运行

开发环境需要 Python 3.13 和 [uv](https://docs.astral.sh/uv/)。项目通过 `.python-version` 固定 Python 版本，并使用 `uv.lock` 锁定依赖。

```powershell
uv sync
uv run python main.py
```

## 测试

```powershell
uv lock --check
uv run pytest -v
```

自动化测试使用测试替身，不会执行真实账号切换、登录或登出，因此不会改变本机 GitHub CLI 认证配置。

## 构建 Windows 单文件 EXE

```powershell
uv sync --dev
uv run pyinstaller --clean --noconfirm GithubDeck.spec
```

构建完成后，程序位于：

```text
dist\GithubDeck.exe
```

发布时将该构建产物重命名为 `GithubDeck-Python-Portable-v0.2.0.exe`，并与其他平台资产一起写入统一的 `SHA256SUMS.txt`。
