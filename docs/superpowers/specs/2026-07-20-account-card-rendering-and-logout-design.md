# 账号卡片渲染修复与精确登出设计

## 目标

修复账号列表中自定义卡片与底层列表项同时绘制造成的文字透出、错位和重叠，并增加“仅登出所选账号”的功能。登出必须经过二次确认，使用 GitHub CLI 的精确主机和用户名参数，并保留现有异步、安全与错误处理边界。

## 渲染问题与修复

当前账号列表使用真实 `QListWidgetItem` 保存文本和 `Account`，并通过 `setItemWidget()` 在其上覆盖 `_AccountCard`。底层列表项仍可能绘制文本，而卡片没有显式启用样式背景绘制，导致文字透出。选中态还会把边框宽度从 1px 改为 2px，使内容区尺寸发生变化，造成文字和徽标跳动。

修复继续保留 `QListWidget` 和真实 `QListWidgetItem`，以维持既有测试和行为接口：

- `QListWidgetItem.text()` 继续保存可访问的账号摘要。
- `Qt.ItemDataRole.UserRole` 继续保存对应 `Account`。
- `count()`、`setCurrentRow()`、`currentItem()` 和选择信号保持不变。
- 列表项底层文字在视觉上设为透明，避免与卡片重复绘制；数据文本不删除。
- `_AccountCard` 显式设置 `Qt.WidgetAttribute.WA_StyledBackground`，确保 QSS 背景完整、稳定地绘制。
- 卡片采用固定或稳定的行高，`QListWidgetItem.sizeHint()` 与卡片高度一致。
- 普通、当前和选中状态均保留相同的 2px 边框占位，只改变颜色，不改变内容区尺寸。
- 当前账号的绿色强调通过卡片内部固定宽度的装饰条表达，不再通过改变左边框宽度表达。
- 登录名、主机和认证文字使用可控的布局伸缩；徽标设置合理最小宽度，并避免挤压主文本区。
- 列表滚动、选择和 disabled 状态继续由 Qt 控件处理。

## 登出入口与启用状态

在主窗口底部操作区增加“登出所选账号”按钮：

- 未选择账号时禁用。
- 选择任意账号后启用，包括当前账号和非当前账号。
- 控制器 busy 时禁用。
- 设备登录进行中或其他 GitHub CLI 操作进行中时遵循同一 busy 状态。
- 按钮使用危险操作色，但不使用夸张的破坏性视觉。
- 账号切换按钮仍只在选择非当前账号时启用，两者的启用规则互不混淆。

## 二次确认

点击“登出所选账号”后显示原生模态确认框：

- 标题：`确认登出账号`。
- 主文案：`确定要从本机 GitHub CLI 登出 <login>（<host>）吗？`
- 说明：`这只会移除本机保存的认证配置，不会撤销 GitHub 端的 OAuth 授权。`
- 操作按钮：`确认登出`和`取消`。
- 默认按钮和初始焦点为`取消`，防止用户按 Enter 意外执行。
- 关闭窗口、按 Escape 或选择取消均不调用控制器。
- 只有明确点击“确认登出”才把当前选中的 `Account` 传给控制器。

确认框不显示或处理 Token，也不提供远端 OAuth 撤销功能。

## GitHub CLI 调用

`GhProcess` 增加：

```text
gh auth logout --hostname <host> --user <login>
```

具体实现继续使用 `QProcess.setProgram()` 和 `QProcess.setArguments()`，不使用 shell 字符串拼接。进程环境继续通过现有 `sanitized_process_environment()` 清除 GitHub Token 环境变量和 `GH_BROWSER`。

本机 GitHub CLI 2.96.0 已确认支持 `--hostname` 和 `--user`。该命令只删除本机 GitHub CLI 保存的认证配置，不撤销 GitHub 端 OAuth 授权。

登出使用与切换相同的 30 秒有限超时。超时和用户关闭应用时继续使用现有 terminate、有限等待和 kill 回退机制。

## 控制器流程

`AuthController.logout(account)` 遵循现有账号切换的安全边界：

1. busy 时拒绝操作。
2. 使用对象身份和内容双重检查，确认 `account` 仍来自当前 `_accounts_snapshot`；过期对象提示“账号列表已变化，请刷新后重试”。
3. 检查 GitHub CLI 可执行文件是否可用。
4. 设置 busy，显示 `正在登出 <login>…`。
5. 调用 `GhProcess.logout(account.host, account.login)`。
6. 如果进程无法启动，恢复 busy 并显示用户友好错误。
7. 登出进程成功后设置确认消息“账号登出成功”，并立即启动 refresh 读取真实状态。
8. refresh 成功后更新账号列表并显示确认消息。
9. refresh 失败时沿用现有安全行为和用户友好提示，不猜测本地账号状态。
10. 登出进程失败时退出 busy、保留当前账号快照和界面列表，并显示“登出账号失败，请检查 GitHub CLI 配置后重试”。

登出不会触发设备登录状态，也不会影响剪贴板会话编号。

## 组件与数据边界

- `GhProcess` 仅负责启动和管理 logout 子进程，不解释凭据。
- `AuthController` 负责账号快照校验、busy 状态、后续刷新和友好消息。
- `MainWindow` 负责按钮启用状态、确认框和把用户确认的 `Account` 传给控制器。
- `Account` 模型保持不变。
- 认证状态解析器保持不变。
- 设置对象和 GitHub CLI 路径校验保持不变。

## 错误与安全边界

- 不调用 `gh auth token` 或 `--show-token`。
- 不读取 Windows Credential Manager。
- 不回显 logout 的 stdout 或 stderr。
- 不记录账号凭据或 Token。
- 不把本机登出描述为远端 OAuth 撤销。
- 不提供整个主机的一键全部登出。
- 不自动选择或登出其他账号。

## 测试与验收

### 渲染测试

- `QListWidgetItem.text()` 和 `UserRole` 继续可用。
- 每个账号项仍有对应 `_AccountCard`。
- 卡片启用 `WA_StyledBackground`。
- 列表项视觉文字透明，避免底层重复绘制。
- 卡片普通、当前和选中状态高度不变。
- 选择变化只改变视觉属性和按钮状态，不改变账号数据。
- 多账号、长登录名、中文状态、窗口缩放和滚动时无文字重叠或卡片错位。

### 进程测试

- `logout(host, login)` 的程序参数精确为 `auth logout --hostname HOST --user LOGIN`。
- logout 使用净化环境和 30 秒超时。
- 进程成功、失败、超时和 shutdown 仍只发出一次完成信号。

### 控制器测试

- busy 时拒绝登出。
- 缺失 GitHub CLI 时拒绝并提示设置路径。
- 过期或伪造账号对象被拒绝。
- 合法账号调用精确 host/login。
- 登出成功后自动 refresh，最终显示“账号登出成功”。
- 登出失败时退出 busy、保留列表并显示友好错误。

### 主窗口测试

- 未选择账号时登出按钮禁用。
- 选择当前或非当前账号时登出按钮启用。
- busy 时登出按钮禁用。
- 取消确认、关闭确认框或按 Escape 不调用控制器。
- 确认框显示账号、主机和本地移除说明，默认按钮为取消。
- 确认后只调用一次 `controller.logout(selected_account)`。

### 完整验收

- 运行 `uv lock --check`。
- 运行完整 pytest 套件。
- 在 Windows 上启动 GUI，使用多账号列表人工确认无文字透出、错位、重叠或选择跳动。
- 人工检查长用户名、当前账号、需重新认证账号、窗口缩放和滚动。
- 使用测试替身验证确认框；自动测试和常规验证不执行真实账号登出。
