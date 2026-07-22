# 账号卡片渲染修复与精确登出实施计划

## 目标与约束

依据 `docs/superpowers/specs/2026-07-20-account-card-rendering-and-logout-design.md`：

- 修复自定义账号卡片与底层 `QListWidgetItem` 重复绘制造成的文字透出、错位和重叠。
- 保留 `QListWidgetItem.text()`、`UserRole`、`count()`、`setCurrentRow()`、`currentItem()` 和现有测试接口。
- 增加仅针对所选账号的本地 GitHub CLI 登出，并要求二次确认。
- 不读取、展示或保存 Token，不执行远端 OAuth 撤销。
- 自动测试不得执行真实账号登出。

## 实施步骤

### 1. 先补充账号卡片稳定性测试

修改：`python-version/tests/test_main_window.py`

扩展现有账号卡片测试，验证：

- 列表项文本仍保存完整账号摘要。
- `UserRole` 仍返回原始 `Account`。
- `count()`、`setCurrentRow()`、`currentItem()` 和 `itemWidget()` 保持可用。
- `_AccountCard` 启用 `Qt.WidgetAttribute.WA_StyledBackground`。
- 列表项前景色透明，但 `text()` 没有被清空。
- 当前与非当前账号的外层卡片高度一致。
- 选择前后卡片高度和 `sizeHint()` 不变。
- 选择仅改变卡片 `selected` 属性，不改变文本和账号数据。

### 2. 先补充主窗口登出交互测试

修改：`python-version/tests/test_main_window.py`

- 为 `FakeController` 增加 `logout(account)` 并记录 `("logout", account)`。
- 验证 `logout_button` 存在且对象名为 `logoutButton`。
- 无选择时禁用。
- 选择当前或非当前账号时均启用。
- busy 时禁用。
- 切换按钮仍只在选择非当前账号时启用。
- 取消、关闭或 Escape 确认框时不调用控制器。
- 确认后只调用一次 `controller.logout(selected_account)`。
- 确认框包含标题、登录名、主机和“只移除本机配置、不撤销 OAuth”的说明。
- 确认框默认按钮、Escape 按钮和初始焦点均为取消。

为避免测试阻塞，主窗口提供 `_confirm_logout(account) -> bool` 测试缝：常规按钮测试 monkeypatch 返回值；另用一个聚焦测试检查原生 `QMessageBox` 的文案、按钮与默认焦点。

### 3. 先补充 GhProcess 登出测试

修改：`python-version/tests/test_gh_process.py`

- 验证 `logout("github.com", "alice")` 启动操作名 `logout`。
- 验证参数精确为：

```python
[
    "auth",
    "logout",
    "--hostname",
    "github.com",
    "--user",
    "alice",
]
```

- 验证调用时读取当前设置中的程序路径。
- 验证继续使用净化环境并移除所有 GitHub Token 变量和 `GH_BROWSER`。
- 验证 logout 启动 30 秒操作超时。
- 验证成功、失败、超时和 shutdown 只产生一次完成信号。

### 4. 先补充控制器登出测试

修改：`python-version/tests/test_controller.py`

为 `FakeProcess` 增加 `logout(host, login)`，覆盖：

- busy 时拒绝登出。
- GitHub CLI 缺失时拒绝并提示设置路径。
- 内容相等但并非快照原对象的账号被拒绝。
- 刷新失败后旧账号对象被拒绝。
- 合法账号传入精确 host/login。
- `process.logout()` 无法启动时恢复 busy 并提示当前有操作。
- 登出进程失败时退出 busy、保留现有账号快照和 UI 列表，并显示友好错误。
- 登出成功后启动 refresh；刷新完成后显示“账号登出成功”。
- 刷新无法启动时提示“操作完成，但无法刷新确认”。

### 5. 修复账号卡片绘制稳定性

修改：

- `python-version/github_cli_manager/main_window.py`
- `python-version/github_cli_manager/app.py`

在 `main_window.py`：

- 定义固定账号卡片高度常量，例如 `_ACCOUNT_CARD_HEIGHT = 76`。
- `_AccountCard` 调用：

```python
self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
```

- 使用固定高度，保证选择和活动状态不改变行高。
- 增加固定宽度的内部 `accountAccent` 装饰控件；活动与非活动状态都占用相同空间，只改变背景色。
- 登录名和元信息布局设置合理伸缩，徽标设置最小宽度，避免长用户名挤压重叠。
- 在 `set_accounts()` 中继续创建带完整文本和 `UserRole` 的真实 `QListWidgetItem`。
- 使用透明前景刷隐藏底层视觉文字，不清空 `item.text()`。
- 列表项 `sizeHint` 使用稳定的固定高度。
- 选择同步只改变动态属性，不修改尺寸和数据。

在 `app.py`：

- `QListWidget::item` 和 `::item:selected` 的文字与背景保持透明。
- 账号卡片所有状态统一使用 2px 边框，只改变边框颜色。
- 移除活动状态改变左边框宽度的规则。
- 为内部 `accountAccent` 增加透明/绿色状态规则。
- 保留现有头像、徽标、授权区和 GitHub 专业控制台样式。

### 6. 实现 GhProcess 精确登出

修改：`python-version/github_cli_manager/gh_process.py`

- 增加 `_LOGOUT_TIMEOUT_MS = 30_000`。
- 增加：

```python
def logout(self, host: str, login: str) -> bool:
    return self._start(
        "logout",
        ["auth", "logout", "--hostname", host, "--user", login],
    )
```

- `_start()` 为 logout 启动 30 秒计时器。
- 继续使用 `QProcess.setProgram()`、`setArguments()` 和 `sanitized_process_environment()`。
- logout 不收集或向 UI 暴露 stdout/stderr。
- 保留现有 terminate、有限等待和 kill 回退机制。

### 7. 实现控制器登出状态机

修改：`python-version/github_cli_manager/controller.py`

- 提取 `_trusted_snapshot_account(account)`，复用 switch 的对象身份和内容校验。
- 在不改变现有行为的前提下让 `switch_account()` 使用该 helper。
- 增加 `logout(account)`：
  1. busy 时返回 False。
  2. 验证账号属于当前快照。
  3. 检查 GitHub CLI 可执行文件。
  4. 设置 busy 并发出“正在登出 <login>…”消息。
  5. 调用精确 host/login 的 process.logout。
  6. 启动失败时恢复 busy 并提示当前有操作。
- `_on_process_finished()` 明确处理 logout：
  - 失败：退出 busy，保留快照，不发出空列表，提示“登出账号失败，请检查 GitHub CLI 配置后重试”。
  - 成功：设置 `_confirmation = "账号登出成功"`，发出刷新确认消息并调用 process.refresh。
  - refresh 无法启动：清除确认、退出 busy、提示无法刷新确认。
- login 的授权码清理和会话逻辑只对 login 生效。

### 8. 增加主窗口登出按钮与二次确认

修改：`python-version/github_cli_manager/main_window.py`

- 导入 `QMessageBox`。
- 在底部账号操作区增加 `self.logout_button = QPushButton("登出所选账号")`，对象名 `logoutButton`，放在切换按钮附近。
- 连接 `_logout_selected()`。
- 统一账号操作按钮更新方法：
  - switch：仅选择非当前账号且非 busy 时启用。
  - logout：选择任意账号且非 busy 时启用。
- `_logout_selected()` 重新读取当前选择；只有 `_confirm_logout(account)` 返回 True 才调用控制器。
- `_confirm_logout()` 使用原生模态 `QMessageBox`：
  - 标题“确认登出账号”。
  - 主文案显示 login 和 host。
  - 说明只移除本机配置，不撤销 OAuth。
  - 自定义“确认登出”危险操作按钮和“取消”按钮。
  - 默认、Escape 和初始焦点均为取消。
  - 只有点击确认按钮才返回 True。

实施时确认当前 PySide6 对自定义 `QPushButton` 的 `setDefaultButton()`/`setEscapeButton()` 重载行为；若 API 不接受自定义按钮，则使用标准 Cancel 按钮并仅自定义确认按钮文本，不降低默认取消的安全要求。

### 9. 增加登出按钮样式

修改：`python-version/github_cli_manager/app.py`

- `logoutButton` 使用克制的红色文字和白色背景。
- hover 使用浅红背景和红色边框。
- disabled 使用现有灰色禁用样式。
- 不把按钮设计成大面积红色主操作，避免误触视觉暗示。

### 10. 更新 README

修改：`python-version/README.md`

- 运行说明加入登出所选账号。
- 支持范围加入“二次确认后精确登出并重新读取真实状态”。
- 从不支持范围删除“注销账号”。
- 明确 logout 只移除本机 GitHub CLI 认证配置，不撤销 GitHub 端 OAuth。
- 安全边界记录使用 host/user 精确参数，不读取 Token。
- 测试说明改为不会执行真实切换、登录或登出。

### 11. 聚焦验证

在任意目录使用绝对项目路径运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"
& "C:\Users\Administrator\.local\bin\uv.exe" run --project "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version" pytest "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version\tests\test_gh_process.py" -q
```

```powershell
$env:QT_QPA_PLATFORM="offscreen"
& "C:\Users\Administrator\.local\bin\uv.exe" run --project "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version" pytest "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version\tests\test_controller.py" -q
```

```powershell
$env:QT_QPA_PLATFORM="offscreen"
& "C:\Users\Administrator\.local\bin\uv.exe" run --project "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version" pytest "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version\tests\test_main_window.py" -q
```

### 12. 完整自动验证

```powershell
& "C:\Users\Administrator\.local\bin\uv.exe" lock --project "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version" --check
```

```powershell
$env:QT_QPA_PLATFORM="offscreen"
& "C:\Users\Administrator\.local\bin\uv.exe" run --project "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version" pytest "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version\tests" -q
```

不新增 lint 或类型工具。实现将修改超过三个文件并包含进程/控制器行为，完成后必须调用独立 verification agent；验证者不得执行真实登出。

### 13. Windows 人工验收

启动：

```powershell
& "C:\Users\Administrator\.local\bin\uv.exe" run --project "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version" python "C:\Users\Administrator\Desktop\LocalPython\github-cli-manager\python-version\main.py"
```

人工检查：

- 多账号卡片完全不透明，无底层文字透出。
- 选择和当前状态不改变卡片高度或内容位置。
- 长登录名、中文状态、徽标和头像不重叠。
- 滚动和窗口缩放不出现错位。
- 无选择时登出禁用；选择当前或非当前账号时启用；busy 时禁用。
- 确认框默认焦点为取消，Escape/关闭/取消不执行登出。
- 确认框清楚说明仅移除本机配置、不撤销 OAuth。
- 真实登出只在用户人工明确确认时执行，成功后列表刷新。

## 预计修改文件

- `python-version/github_cli_manager/app.py`
- `python-version/github_cli_manager/main_window.py`
- `python-version/github_cli_manager/controller.py`
- `python-version/github_cli_manager/gh_process.py`
- `python-version/tests/test_main_window.py`
- `python-version/tests/test_controller.py`
- `python-version/tests/test_gh_process.py`
- `python-version/README.md`

不计划修改：

- `models.py`
- `auth_parser.py`
- `gh_settings.py`
- `gh_settings_dialog.py`

## 风险控制

- 不替换 `QListWidget`，避免破坏测试和选择接口。
- 不删除或清空 `QListWidgetItem.text()`。
- 不改变选中与未选中边框宽度。
- 不使用活动状态左边框宽度变化。
- 不在缺少 `--hostname` 或 `--user` 时调用 logout。
- 不构造 shell 命令字符串。
- 不展示 logout 原始输出。
- 不调用 `gh auth token`。
- 不声称或实现远端 OAuth 撤销。
- 自动测试不执行真实登出。
