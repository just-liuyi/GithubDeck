# GitHub 专业控制台 PySide6 实施计划

## 目标与边界

依据 `docs/superpowers/specs/2026-07-20-github-professional-console-visual-design.md` 重写 PySide6 外观，不修改账号读取、切换、设备登录、剪贴板重试、异步校验或进程控制逻辑。

实现必须保留测试和调用方依赖的公开控件属性，尤其是 `account_list` 的 `QListWidget` 语义、各操作按钮、授权字段、`authorization_widget` 和 `status_label`。

## 样式架构决定

暂不新增独立样式模块。当前只有主窗口和设置对话框需要共享视觉语言，在 `github_cli_manager/app.py` 定义并应用一份应用级 Qt Style Sheet 足够直接，也避免过早抽象。

`main_window.py` 和 `gh_settings_dialog.py` 只设置对象名与少量动态属性。若基础 `QListWidget` 项样式不能表达账号卡片，则在 `main_window.py` 内增加一个无业务状态的轻量展示组件；不建立新的跨文件组件层。

## 实施步骤

### 1. 先补充主窗口视觉结构测试

修改：`python-version/tests/test_main_window.py`

- 新增测试，确认现有关键对象名和公开属性不变：`accountList`、`settingsButton`、`refreshButton`、`switchButton`、`loginButton`、`cancelButton`、`authorizationWidget`、`authorizationUrl`、`authorizationCode`、`copyUrlButton`、`copyCodeButton`、`statusLabel`。
- 为新布局约定稳定对象名：`mainContainer`、`appHeader`、`pageHeader`、`accountSection`、`accountCountLabel`、`bottomActions`、`statusBar`。
- 扩展账号渲染测试，覆盖当前账号、普通已认证账号、需重新认证和未知状态；继续使用 `count()`、`item(index).text()`、`setCurrentRow()` 和 `UserRole` 验证基础语义。
- 若使用账号卡片展示组件，验证当前、认证和选中动态属性，而不测试像素值。
- 新增账号数量标签测试，确保 `set_accounts()` 后数量同步，但列表本身仍是行为数据源。
- 新增授权面板标题和安全说明测试，同时保留现有只读字段、显式复制、取消和清空行为测试。
- 新增 busy 与 error 动态属性断言；把旧错误色断言更新为规范中的 `#cf222e`，同时继续验证状态文本。

运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"; uv run pytest tests/test_main_window.py -q
```

预期：新测试先因缺少视觉结构而失败，现有行为测试仍说明兼容边界。

### 2. 先补充设置对话框视觉结构测试

修改：`python-version/tests/test_gh_settings_dialog.py`

- 验证既有对象名继续存在：`ghPathEdit`、`browseButton`、`resetButton`、`ghSettingsStatus`、`ghSourceLabel`。
- 为新布局约定：`ghSettingsHeader`、`ghSettingsPathCard`、`ghSettingsHelpCard`。
- 为状态标签约定 `state` 动态属性：`idle`、`validating`、`success`、`error`。
- 扩展保存校验测试：校验期间选择、恢复默认和保存禁用，取消保持启用，状态为 `validating`。
- 扩展失败测试：对话框保持打开、设置不变、控件恢复、状态为 `error`。
- 不改动已覆盖的取消晚回调、恢复默认和合法路径保存语义。

运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"; uv run pytest tests/test_gh_settings_dialog.py -q
```

### 3. 增加集中式应用样式

修改：`python-version/github_cli_manager/app.py`

- 定义 `GITHUB_CONSOLE_STYLE_SHEET` 常量。
- 在设置应用组织名和应用名之后、构造窗口之前调用 `app.setStyleSheet(...)`。
- 使用规范色值：背景 `#f6f8fa`、深色标题 `#24292f`、表面 `#ffffff`、边框 `#d0d7de`、正文 `#1f2328`、次要文字 `#656d76`、主操作 `#1f883d`、选择态 `#0969da`、错误 `#cf222e`。
- 样式按对象名覆盖标题区、按钮、账号列表和卡片、授权面板、状态栏及设置对话框；QSS 不承载行为。
- 保持设置对象、进程、控制器、初始刷新和事件循环构造顺序不变。

可选新增：`python-version/tests/test_app_style.py`

- 只验证关键选择器与规范色存在，不建立脆弱的完整字符串或像素快照测试。

### 4. 重构主窗口布局，保留公开接口

修改：`python-version/github_cli_manager/main_window.py`

- 将默认尺寸调整为约 `820 × 620`，设置合理最小尺寸防止中文文本和按钮重叠。
- 以 `mainContainer` 作为中央容器，依次组织：
  1. `appHeader` 深色应用标题区；
  2. `pageHeader` 页面标题、说明、刷新和设置；
  3. `accountSection` 账号数量和列表；
  4. 账号列表下方的授权面板；
  5. `bottomActions` 登录、切换和取消；
  6. `statusBar` 状态点和现有 `status_label`。
- 保留所有现有公开字段、对象名、tooltip、accessibleName 和信号连接。
- `authorization_url_edit` 与 `authorization_code_edit` 继续只读；授权区域仍只在存在设备码时显示。
- 不修改 `_copy_authorization_url()`、`_copy_authorization_code()`、`_copy_text_with_retry()` 的重试次数、延迟和旧会话防护。
- 不修改 `set_login_active()` 的会话编号增长、取消显示和授权码清理。

运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"; uv run pytest tests/test_main_window.py -q
```

### 5. 实现账号卡片展示但保留 QListWidget 语义

修改：`python-version/github_cli_manager/main_window.py`

- 仅在需要时添加 `_AccountCard(QWidget)`，负责头像缩写、登录名、主机、认证状态和当前账号徽标。
- 每个账号仍创建真实 `QListWidgetItem`：保留原文本、`Qt.ItemDataRole.UserRole` 中的 `Account`、当前账号粗体及原排序。
- 使用 `setItemWidget(item, card)` 附加展示组件，设置合理 `sizeHint`；现有 `count()`、`item().text()`、`setCurrentRow()`、`currentItem()` 和 `_selected_account()` 不变。
- 选择变化时同步卡片 `selected` 动态属性；当前状态用 `active`，认证状态用 `authState`。属性变化后仅刷新控件样式，不改变模型或控制器状态。
- `set_accounts()` 同步 `accountCountLabel`，但不把标签作为行为数据源。
- `set_busy()` 保持原禁用范围；取消按钮仍由登录活动状态控制，并在登录忙碌时可用。只额外设置视觉动态属性。
- `show_message()` 继续写入原 `status_label`；增加 `error` 属性和批准的红色显示，不解析或显示 GitHub CLI 原始输出。

### 6. 重构 GitHub CLI 设置对话框布局

修改：`python-version/github_cli_manager/gh_settings_dialog.py`

- 调整窗口尺寸与最小尺寸，建立 `ghSettingsHeader`、`ghSettingsPathCard`、`ghSettingsHelpCard` 三个视觉层级。
- 保留 `path_edit`、`browse_button`、`reset_button`、`status_label`、`button_box` 的公开属性和既有对象名。
- 标题区加入简短说明；路径卡片放置只读路径、选择和恢复默认；帮助卡片保留官方链接、Windows MSI 指引和 `gh_2.96.0_windows_amd64.msi` 文案。
- 初始状态设为 `idle`；保存时设为 `validating`；失败为 `error`；成功接受前设为 `success`，每次属性变化刷新样式。
- 严格保留：
  - `_set_candidate()` 的路径归一化；
  - 文件选择器参数；
  - `_set_controls_enabled()` 只影响选择、恢复默认和保存；
  - 校验期间取消按钮可用；
  - `reject()` 清除待保存并取消校验；
  - 只有有效校验结果才能写设置并接受对话框；
  - 默认路径使用 `reset()`，自定义路径使用 `set_path()`。

运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"; uv run pytest tests/test_gh_settings_dialog.py -q
```

### 7. 聚焦回归验证

在 `python-version` 目录运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"; uv run pytest tests/test_main_window.py tests/test_gh_settings_dialog.py -q
```

若新增样式常量测试，一并运行：

```powershell
$env:QT_QPA_PLATFORM="offscreen"; uv run pytest tests/test_app_style.py -q
```

任何失败先判断是视觉结构遗漏还是行为回归；不得通过删除既有行为断言制造通过。

### 8. 完整自动验证

在 `python-version` 目录运行：

```powershell
uv lock --check
$env:QT_QPA_PLATFORM="offscreen"; uv run pytest -q
```

项目当前未配置 lint 或类型检查工具，不引入额外工具。

由于将修改至少三个文件，完成实现后还需调用独立 verification agent，传入原始视觉重写请求、全部变更文件和实施方式。只有 verifier 给出 PASS 后才能报告完成；PARTIAL 必须明确说明无法验证的部分。

### 9. Windows 人工界面验收

运行：

```powershell
uv run python main.py
```

人工检查：

- 默认和最小窗口尺寸下没有裁切、重叠或按钮溢出。
- 深色标题区、灰白表面、绿色主操作、蓝色选中态和红色错误态符合确认稿。
- 空账号、单账号、多账号和滚动列表均正常。
- 当前账号徽标、认证状态与列表选择清晰，切换按钮只在选中非当前账号时启用。
- 登录设备码出现后授权面板在列表下方展开；网址和授权码只读且只在点击后复制。
- 登录忙碌时常规操作禁用，取消仍可用；取消或结束后面板收起并清空授权码。
- 设置对话框路径、官方链接和 MSI 文案无裁切；保存校验期间取消可用；成功、失败和取消行为与测试一致。
- 所有状态文本保持用户友好，不展示原始命令输出或 Token。

## 预计变更文件

- `python-version/github_cli_manager/app.py`
- `python-version/github_cli_manager/main_window.py`
- `python-version/github_cli_manager/gh_settings_dialog.py`
- `python-version/tests/test_main_window.py`
- `python-version/tests/test_gh_settings_dialog.py`
- 可选：`python-version/tests/test_app_style.py`

不计划修改：

- `controller.py`
- `gh_process.py`
- `gh_settings.py`
- `models.py`
- `auth_parser.py`

## 主要风险与控制

- **列表接口破坏：** 始终保留真实 `QListWidgetItem` 的文本和 `UserRole`，展示组件只通过 `setItemWidget()` 附加。
- **选择态不刷新：** 在 `itemSelectionChanged` 后同步卡片动态属性并重新 polish。
- **取消按钮被误禁用：** 不把取消按钮加入 `set_busy()` 或设置对话框 `_set_controls_enabled()` 的禁用集合。
- **剪贴板旧回调复活：** 不修改登录会话编号和 `_copy_text_with_retry()` 的防护条件。
- **直接构造窗口时没有应用 QSS：** 测试关注对象名、属性和行为；样式常量另做轻量选择器测试，不做截图断言。
