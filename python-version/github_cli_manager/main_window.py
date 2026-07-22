from PySide6.QtCore import QSize, Qt, QTimer
from PySide6.QtGui import QBrush, QColor, QIcon
from PySide6.QtWidgets import (
    QAbstractItemView,
    QApplication,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from .controller import AuthController
from .gh_process import GhProcess
from .gh_settings import GhExecutableSettings
from .gh_settings_dialog import GhSettingsDialog
from .models import Account, AuthState


_AUTH_STATE_LABELS = {
    AuthState.AUTHENTICATED: "已认证",
    AuthState.NEEDS_LOGIN: "需重新认证",
    AuthState.UNKNOWN: "认证状态未知",
}
_AUTH_STATE_PROPERTIES = {
    AuthState.AUTHENTICATED: "authenticated",
    AuthState.NEEDS_LOGIN: "needs-login",
    AuthState.UNKNOWN: "unknown",
}
_DEVICE_LOGIN_URL = "https://github.com/login/device"
_ACCOUNT_CARD_HEIGHT = 76


def _refresh_style(widget: QWidget) -> None:
    style = widget.style()
    style.unpolish(widget)
    style.polish(widget)
    widget.update()


class _AccountCard(QWidget):
    def __init__(self, account: Account, parent=None) -> None:
        super().__init__(parent)
        self.setObjectName("accountCard")
        self.setAttribute(Qt.WidgetAttribute.WA_StyledBackground, True)
        self.setProperty("active", account.active)
        self.setProperty("selected", False)
        auth_state = _AUTH_STATE_PROPERTIES[account.state]
        self.setProperty("authState", auth_state)
        self.setFixedHeight(_ACCOUNT_CARD_HEIGHT)

        layout = QHBoxLayout(self)
        layout.setContentsMargins(8, 9, 13, 9)
        layout.setSpacing(11)

        accent = QWidget(self)
        accent.setObjectName("accountAccent")
        accent.setProperty("active", account.active)
        accent.setFixedWidth(4)
        layout.addWidget(accent)

        avatar = QLabel(self._initials(account.login), self)
        avatar.setObjectName("accountAvatar")
        avatar.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(avatar)

        copy_layout = QVBoxLayout()
        copy_layout.setSpacing(2)
        login = QLabel(account.login, self)
        login.setObjectName("accountLogin")
        copy_layout.addWidget(login)
        meta = QLabel(f"{account.host}  ·  {_AUTH_STATE_LABELS[account.state]}", self)
        meta.setObjectName("accountMeta")
        copy_layout.addWidget(meta)
        layout.addLayout(copy_layout, 1)

        badge = QLabel("当前账号" if account.active else _AUTH_STATE_LABELS[account.state], self)
        badge.setObjectName("accountBadge")
        badge.setProperty("active", account.active)
        badge.setProperty("authState", auth_state)
        badge.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(badge)

    @staticmethod
    def _initials(login: str) -> str:
        parts = [part for part in login.replace("-", " ").replace("_", " ").split() if part]
        if len(parts) > 1:
            return "".join(part[0] for part in parts[:2]).upper()
        return login[:2].upper()

    def set_selected(self, selected: bool) -> None:
        if self.property("selected") == selected:
            return
        self.setProperty("selected", selected)
        _refresh_style(self)


class MainWindow(QMainWindow):
    def __init__(
        self,
        controller: AuthController | None = None,
        settings: GhExecutableSettings | None = None,
        settings_dialog_type=GhSettingsDialog,
        app_icon: QIcon | None = None,
    ) -> None:
        super().__init__()
        if controller is None:
            self._gh_settings = settings or GhExecutableSettings()
            self.controller = AuthController(
                GhProcess(self._gh_settings, parent=self), parent=self
            )
        else:
            controller_settings = getattr(controller, "gh_settings", None)
            if settings is not None and controller_settings is not None and settings is not controller_settings:
                raise ValueError("controller 和 settings 必须使用同一配置对象")
            self._gh_settings = settings or controller_settings or GhExecutableSettings()
            self.controller = controller
        self._settings_dialog_type = settings_dialog_type
        self.setWindowTitle("GithubDeck")
        if app_icon is not None:
            self.setWindowIcon(app_icon)
        self.resize(820, 620)
        self.setMinimumSize(720, 560)

        central = QWidget(self)
        central.setObjectName("mainContainer")
        root_layout = QVBoxLayout(central)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        app_header = QWidget(central)
        app_header.setObjectName("appHeader")
        app_header.setFixedHeight(48)
        app_header_layout = QHBoxLayout(app_header)
        app_header_layout.setContentsMargins(20, 0, 20, 0)
        app_header_layout.setSpacing(10)
        app_mark = QLabel(app_header)
        app_mark.setObjectName("appMark")
        app_mark.setAlignment(Qt.AlignmentFlag.AlignCenter)
        app_mark.setFixedSize(24, 24)
        if app_icon is not None:
            app_mark.setPixmap(app_icon.pixmap(24, 24))
        app_header_layout.addWidget(app_mark)
        app_title = QLabel("GithubDeck", app_header)
        app_title.setObjectName("appTitle")
        app_header_layout.addWidget(app_title)
        app_header_layout.addStretch()
        root_layout.addWidget(app_header)

        content = QWidget(central)
        content_layout = QVBoxLayout(content)
        content_layout.setContentsMargins(28, 22, 28, 18)
        content_layout.setSpacing(16)

        page_header = QWidget(content)
        page_header.setObjectName("pageHeader")
        page_header_layout = QHBoxLayout(page_header)
        page_header_layout.setContentsMargins(0, 0, 0, 0)
        page_header_layout.setSpacing(12)
        heading_layout = QVBoxLayout()
        heading_layout.setSpacing(3)
        page_title = QLabel("GitHub 账号", page_header)
        page_title.setObjectName("pageTitle")
        heading_layout.addWidget(page_title)
        page_description = QLabel("管理本机 GitHub CLI 中已认证的账号", page_header)
        page_description.setObjectName("pageDescription")
        heading_layout.addWidget(page_description)
        page_header_layout.addLayout(heading_layout)
        page_header_layout.addStretch()
        self.refresh_button = QPushButton("刷新", page_header)
        self.refresh_button.setObjectName("refreshButton")
        page_header_layout.addWidget(self.refresh_button)
        self.settings_button = QPushButton("GitHub CLI 设置", page_header)
        self.settings_button.setObjectName("settingsButton")
        self.settings_button.setToolTip("GitHub CLI 设置")
        self.settings_button.setAccessibleName("GitHub CLI 设置")
        page_header_layout.addWidget(self.settings_button)
        content_layout.addWidget(page_header)

        account_section = QWidget(content)
        account_section.setObjectName("accountSection")
        account_section_layout = QVBoxLayout(account_section)
        account_section_layout.setContentsMargins(14, 13, 14, 14)
        account_section_layout.setSpacing(10)
        account_heading = QHBoxLayout()
        account_heading.setSpacing(8)
        account_label = QLabel("已认证账号", account_section)
        account_label.setObjectName("accountSectionLabel")
        account_heading.addWidget(account_label)
        self.account_count_label = QLabel("0 个账号", account_section)
        self.account_count_label.setObjectName("accountCountLabel")
        account_heading.addWidget(self.account_count_label)
        account_heading.addStretch()
        account_section_layout.addLayout(account_heading)

        self.account_list = QListWidget(account_section)
        self.account_list.setObjectName("accountList")
        self.account_list.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self.account_list.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.account_list.setSpacing(7)
        self.account_list.itemSelectionChanged.connect(self._update_switch_button)
        account_section_layout.addWidget(self.account_list, 1)
        content_layout.addWidget(account_section, 1)

        self.authorization_widget = QWidget(content)
        self.authorization_widget.setObjectName("authorizationWidget")
        authorization_layout = QVBoxLayout(self.authorization_widget)
        authorization_layout.setContentsMargins(15, 13, 15, 13)
        authorization_layout.setSpacing(9)
        authorization_title = QLabel("在浏览器中完成 GitHub 授权", self.authorization_widget)
        authorization_title.setObjectName("authorizationTitle")
        authorization_layout.addWidget(authorization_title)
        authorization_description = QLabel(
            "打开官方设备授权页并输入一次性授权码。应用不会读取或保存你的 Token。",
            self.authorization_widget,
        )
        authorization_description.setObjectName("authorizationDescription")
        authorization_description.setWordWrap(True)
        authorization_layout.addWidget(authorization_description)
        url_layout = QHBoxLayout()
        url_layout.setSpacing(8)
        url_layout.addWidget(QLabel("授权网址", self.authorization_widget))
        self.authorization_url_edit = QLineEdit(_DEVICE_LOGIN_URL, self.authorization_widget)
        self.authorization_url_edit.setObjectName("authorizationUrl")
        self.authorization_url_edit.setReadOnly(True)
        self.copy_url_button = QPushButton("复制网址", self.authorization_widget)
        self.copy_url_button.setObjectName("copyUrlButton")
        url_layout.addWidget(self.authorization_url_edit, 1)
        url_layout.addWidget(self.copy_url_button)
        authorization_layout.addLayout(url_layout)
        code_layout = QHBoxLayout()
        code_layout.setSpacing(8)
        code_layout.addWidget(QLabel("授权码", self.authorization_widget))
        self.authorization_code_edit = QLineEdit(self.authorization_widget)
        self.authorization_code_edit.setObjectName("authorizationCode")
        self.authorization_code_edit.setReadOnly(True)
        self.copy_code_button = QPushButton("复制授权码", self.authorization_widget)
        self.copy_code_button.setObjectName("copyCodeButton")
        code_layout.addWidget(self.authorization_code_edit, 1)
        code_layout.addWidget(self.copy_code_button)
        authorization_layout.addLayout(code_layout)
        self.authorization_widget.hide()
        content_layout.addWidget(self.authorization_widget)

        bottom_actions = QWidget(content)
        bottom_actions.setObjectName("bottomActions")
        buttons = QHBoxLayout(bottom_actions)
        buttons.setContentsMargins(0, 0, 0, 0)
        buttons.setSpacing(8)
        self.switch_button = QPushButton("切换到所选账号", bottom_actions)
        self.switch_button.setObjectName("switchButton")
        self.logout_button = QPushButton("登出所选账号", bottom_actions)
        self.logout_button.setObjectName("logoutButton")
        self.login_button = QPushButton("登录新账号", bottom_actions)
        self.login_button.setObjectName("loginButton")
        self.cancel_button = QPushButton("取消登录", bottom_actions)
        self.cancel_button.setObjectName("cancelButton")
        self.cancel_button.hide()
        buttons.addWidget(self.switch_button)
        buttons.addWidget(self.logout_button)
        buttons.addStretch()
        buttons.addWidget(self.cancel_button)
        buttons.addWidget(self.login_button)
        content_layout.addWidget(bottom_actions)
        root_layout.addWidget(content, 1)

        status_bar = QWidget(central)
        status_bar.setObjectName("statusBar")
        status_bar.setFixedHeight(38)
        status_layout = QHBoxLayout(status_bar)
        status_layout.setContentsMargins(18, 0, 18, 0)
        status_layout.setSpacing(8)
        self.status_dot = QLabel("●", status_bar)
        self.status_dot.setObjectName("statusDot")
        status_layout.addWidget(self.status_dot)
        self.status_label = QLabel("准备就绪", status_bar)
        self.status_label.setObjectName("statusLabel")
        self.status_label.setWordWrap(True)
        self.status_label.setProperty("error", False)
        status_layout.addWidget(self.status_label, 1)
        status_version = QLabel("GitHub CLI", status_bar)
        status_version.setObjectName("statusVersion")
        status_layout.addWidget(status_version)
        root_layout.addWidget(status_bar)
        self.setCentralWidget(central)

        self.refresh_button.clicked.connect(self.controller.refresh)
        self.settings_button.clicked.connect(self._open_gh_settings)
        self.login_button.clicked.connect(self._login)
        self.cancel_button.clicked.connect(self.controller.cancel)
        self.copy_url_button.clicked.connect(self._copy_authorization_url)
        self.copy_code_button.clicked.connect(self._copy_authorization_code)
        self.switch_button.clicked.connect(self._switch_selected)
        self.logout_button.clicked.connect(self._logout_selected)
        self.controller.accounts_changed.connect(self.set_accounts)
        self.controller.busy_changed.connect(self.set_busy)
        self.controller.message_changed.connect(self.show_message)
        self.controller.authorization_code.connect(self.set_authorization_code)
        self.controller.login_active.connect(self.set_login_active)
        self._login_active = False
        self._login_session = 0
        self.setProperty("busy", False)
        self._update_switch_button()

    def set_accounts(self, accounts: list[Account]) -> None:
        self.account_list.clear()
        for account in accounts:
            details = [_AUTH_STATE_LABELS[account.state]]
            if account.active:
                details.append("当前")
            item = QListWidgetItem(
                f"{account.login}  ·  {account.host}（{'，'.join(details)}）"
            )
            item.setData(Qt.ItemDataRole.UserRole, account)
            if account.active:
                font = item.font()
                font.setBold(True)
                item.setFont(font)
            card = _AccountCard(account, self.account_list)
            item.setForeground(QBrush(QColor(0, 0, 0, 0)))
            item.setSizeHint(QSize(0, _ACCOUNT_CARD_HEIGHT))
            self.account_list.addItem(item)
            self.account_list.setItemWidget(item, card)
        self.account_count_label.setText(f"{len(accounts)} 个账号")
        self._update_switch_button()

    def set_busy(self, busy: bool) -> None:
        self.account_list.setEnabled(not busy)
        self.refresh_button.setEnabled(not busy)
        self.login_button.setEnabled(not busy)
        self.settings_button.setEnabled(not busy)
        self.setProperty("busy", busy)
        self._update_switch_button()

    def set_login_active(self, active: bool) -> None:
        if active != self._login_active:
            self._login_session += 1
        self._login_active = active
        self.cancel_button.setVisible(active)
        self.cancel_button.setEnabled(active)
        if not active:
            self.set_authorization_code("")

    def set_authorization_code(self, code: str) -> None:
        self.authorization_code_edit.setText(code)
        self.authorization_widget.setVisible(bool(code))

    def show_message(self, message: str, is_error: bool) -> None:
        self.status_label.setText(message)
        self.status_label.setProperty("error", is_error)
        self.status_dot.setProperty("error", is_error)
        _refresh_style(self.status_label)
        _refresh_style(self.status_dot)

    def closeEvent(self, event) -> None:
        self.controller.shutdown()
        super().closeEvent(event)

    def _selected_account(self) -> Account | None:
        item = self.account_list.currentItem()
        return item.data(Qt.ItemDataRole.UserRole) if item is not None else None

    def _open_gh_settings(self) -> None:
        dialog = self._settings_dialog_type(self._gh_settings, parent=self)
        dialog.exec()

    def _login(self) -> None:
        self.controller.login()

    def _copy_authorization_url(self) -> None:
        self._copy_text_with_retry(
            self.authorization_url_edit,
            self.authorization_url_edit.text(),
            "授权网址已复制",
            3,
            self._login_session,
        )

    def _copy_authorization_code(self) -> None:
        code = self.authorization_code_edit.text()
        if code:
            self._copy_text_with_retry(
                self.authorization_code_edit,
                code,
                "授权码已复制",
                3,
                self._login_session,
            )

    def _copy_text_with_retry(
        self,
        source: QLineEdit,
        text: str,
        success_message: str,
        attempts: int,
        login_session: int,
    ) -> None:
        if (
            not self._login_active
            or self._login_session != login_session
            or source.text() != text
        ):
            return
        clipboard = QApplication.clipboard()
        clipboard.setText(text)
        if clipboard.text() == text:
            self.show_message(success_message, False)
        elif attempts > 1:
            QTimer.singleShot(
                75,
                lambda: self._copy_text_with_retry(
                    source, text, success_message, attempts - 1, login_session
                ),
            )
        else:
            self.show_message("无法访问系统剪贴板，请稍后重试", True)

    def _switch_selected(self) -> None:
        account = self._selected_account()
        if account is not None and not account.active:
            self.controller.switch_account(account)

    def _logout_selected(self) -> None:
        account = self._selected_account()
        if account is not None and self._confirm_logout(account):
            self.controller.logout(account)

    def _confirm_logout(self, account: Account) -> bool:
        dialog = QMessageBox(self)
        dialog.setWindowTitle("确认登出账号")
        dialog.setIcon(QMessageBox.Icon.Warning)
        dialog.setText(
            f"确定要从本机 GitHub CLI 登出 {account.login}（{account.host}）吗？"
        )
        dialog.setInformativeText(
            "这只会移除本机保存的认证配置，不会撤销 GitHub 端的 OAuth 授权。"
        )
        confirm = dialog.addButton("确认登出", QMessageBox.ButtonRole.DestructiveRole)
        cancel = dialog.addButton("取消", QMessageBox.ButtonRole.RejectRole)
        dialog.setDefaultButton(cancel)
        dialog.setEscapeButton(cancel)
        cancel.setFocus()
        dialog.exec()
        return dialog.clickedButton() is confirm

    def _sync_account_card_selection(self) -> None:
        current = self.account_list.currentItem()
        for index in range(self.account_list.count()):
            item = self.account_list.item(index)
            card = self.account_list.itemWidget(item)
            if isinstance(card, _AccountCard):
                card.set_selected(item is current)

    def _update_switch_button(self) -> None:
        self._sync_account_card_selection()
        account = self._selected_account()
        enabled = (
            not self.controller.busy
            and self.account_list.isEnabled()
            and account is not None
            and not account.active
        )
        self.switch_button.setEnabled(enabled)
        self.logout_button.setEnabled(
            not self.controller.busy
            and self.account_list.isEnabled()
            and account is not None
        )
