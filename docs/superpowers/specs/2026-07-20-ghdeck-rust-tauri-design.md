# GhDeck Rust + Tauri 版本设计

## 目标

在 `rust-version` 中实现与 Python 版功能等价的 **GhDeck** 桌面应用：

- 技术栈：Rust（Tauri 2 后端）+ React + TypeScript + HTML/CSS
- 功能：刷新、切换、设备登录、取消、精确登出、`gh.exe` 设置与校验
- 视觉：复刻 GitHub 专业控制台样式与中文文案
- 安全：不读取/展示 Token，参数数组调用 `gh`，净化环境变量

## 架构

```text
React UI  --invoke/events-->  Tauri commands  -->  AuthController
                                                    |
                                                    v
                                               GhProcess (tokio)
                                                    |
                                                    v
                                               gh.exe (本地)
```

- `models`：`Account` / `AuthState`
- `auth_parser`：解析 `gh auth status --json hosts`
- `settings`：持久化 `gh.exe` 路径并校验版本
- `gh_process`：异步子进程、超时、设备码提取
- `controller`：串行操作、快照信任、成功后刷新确认
- 前端：主窗口 + 设置对话框 + 授权面板

## 安全边界

与 Python 版一致：不调用 `gh auth token`、不读 Credential Manager、不展示原始输出、登出仅本机配置。
