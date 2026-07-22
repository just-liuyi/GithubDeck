from PySide6.QtCore import QObject, Qt, Signal
from PySide6.QtGui import QColor, QIcon, QImage
from PySide6.QtWidgets import QApplication, QLabel

from github_cli_manager.main_window import MainWindow
from github_cli_manager.models import Account, AuthState


class FakeController(QObject):
    accounts_changed = Signal(list)
    busy_changed = Signal(bool)
    message_changed = Signal(str, bool)
    authorization_code = Signal(str)
    login_active = Signal(bool)

    def __init__(self) -> None:
        super().__init__()
        self.busy = False
        self.calls: list[tuple] = []

    def refresh(self) -> bool:
        self.calls.append(("refresh",))
        return True

    def login(self) -> bool:
        self.calls.append(("login",))
        return True

    def switch_account(self, account: Account) -> bool:
        self.calls.append(("switch", account))
        return True

    def logout(self, account: Account) -> bool:
        self.calls.append(("logout", account))
        return True

    def cancel(self) -> bool:
        self.calls.append(("cancel",))
        self.login_active.emit(False)
        return True

    def shutdown(self) -> None:
        self.calls.append(("shutdown",))


def test_window_renders_accounts_and_switches_selected(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    accounts = [
        Account("github.com", "alice", True),
        Account("github.com", "bob", False),
    ]

    controller.accounts_changed.emit(accounts)
    assert window.account_list.count() == 2
    assert "当前" in window.account_list.item(0).text()
    assert not window.switch_button.isEnabled()

    window.account_list.setCurrentRow(1)
    assert window.switch_button.isEnabled()
    qtbot.mouseClick(window.switch_button, Qt.MouseButton.LeftButton)
    assert controller.calls == [("switch", accounts[1])]


def test_busy_disables_all_actions(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    controller.accounts_changed.emit([Account("github.com", "bob", False)])
    window.account_list.setCurrentRow(0)

    controller.busy = True
    controller.busy_changed.emit(True)

    assert not window.refresh_button.isEnabled()
    assert not window.switch_button.isEnabled()
    assert not window.login_button.isEnabled()
    assert not window.account_list.isEnabled()
    assert not window.settings_button.isEnabled()


def test_window_normalizes_auth_state_and_allows_login_cancel(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    window.show()
    controller.accounts_changed.emit(
        [Account("github.com", "alice", state=AuthState.NEEDS_LOGIN)]
    )

    assert "需重新认证" in window.account_list.item(0).text()
    controller.busy = True
    controller.busy_changed.emit(True)
    controller.login_active.emit(True)
    assert window.cancel_button.isVisible()
    qtbot.mouseClick(window.cancel_button, Qt.MouseButton.LeftButton)
    assert controller.calls == [("cancel",)]


def test_login_calls_controller_without_browser_selection(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)

    qtbot.mouseClick(window.login_button, Qt.MouseButton.LeftButton)

    assert controller.calls == [("login",)]


def test_mismatched_controller_and_settings_are_rejected() -> None:
    controller = FakeController()
    controller.gh_settings = object()

    try:
        MainWindow(controller, settings=object())
    except ValueError as exc:
        assert str(exc) == "controller 和 settings 必须使用同一配置对象"
    else:
        raise AssertionError("应拒绝不一致的配置对象")


def test_settings_button_opens_github_cli_settings(qtbot) -> None:
    calls: list[object] = []

    class FakeDialog:
        def __init__(self, settings, parent=None) -> None:
            calls.extend([settings, parent])

        def exec(self) -> None:
            calls.append("exec")

    controller = FakeController()
    window = MainWindow(controller, settings_dialog_type=FakeDialog)
    qtbot.addWidget(window)

    qtbot.mouseClick(window.settings_button, Qt.MouseButton.LeftButton)

    assert calls[0] is window._gh_settings
    assert calls[1:] == [window, "exec"]
    assert window.settings_button.toolTip() == "GitHub CLI 设置"


def test_authorization_url_and_code_are_explicitly_copied_and_cleared(qtbot, monkeypatch) -> None:
    class FakeClipboard:
        value = ""

        def setText(self, text: str) -> None:
            self.value = text

        def text(self) -> str:
            return self.value

    clipboard = FakeClipboard()
    monkeypatch.setattr(QApplication, "clipboard", lambda: clipboard)
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)

    window.show()
    controller.login_active.emit(True)
    controller.authorization_code.emit("ABCD-EF12")
    assert window.authorization_widget.isVisible()
    assert window.authorization_url_edit.text() == "https://github.com/login/device"
    assert window.authorization_url_edit.isReadOnly()
    assert window.authorization_code_edit.text() == "ABCD-EF12"
    assert clipboard.text() == ""

    qtbot.mouseClick(window.copy_url_button, Qt.MouseButton.LeftButton)
    assert clipboard.text() == "https://github.com/login/device"
    assert window.status_label.text() == "授权网址已复制"
    qtbot.mouseClick(window.copy_code_button, Qt.MouseButton.LeftButton)
    assert clipboard.text() == "ABCD-EF12"
    assert window.status_label.text() == "授权码已复制"

    controller.login_active.emit(False)
    assert window.authorization_code_edit.text() == ""
    assert window.authorization_widget.isHidden()


def test_copy_retry_succeeds_on_later_attempt(qtbot, monkeypatch) -> None:
    class RetryingClipboard:
        calls: list[str] = []
        value = ""

        def setText(self, text: str) -> None:
            self.calls.append(text)
            if len(self.calls) == 2:
                self.value = text

        def text(self) -> str:
            return self.value

    callbacks = []
    clipboard = RetryingClipboard()
    monkeypatch.setattr(QApplication, "clipboard", lambda: clipboard)
    monkeypatch.setattr(
        "github_cli_manager.main_window.QTimer.singleShot",
        lambda _delay, callback: callbacks.append(callback),
    )
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    controller.login_active.emit(True)
    controller.authorization_code.emit("ABCD-EF12")

    window._copy_authorization_code()
    callbacks.pop()()

    assert clipboard.calls == ["ABCD-EF12", "ABCD-EF12"]
    assert window.status_label.text() == "授权码已复制"


def test_stale_copy_retry_is_ignored_after_login_ends(qtbot, monkeypatch) -> None:
    class UnavailableClipboard:
        calls: list[str] = []

        def setText(self, text: str) -> None:
            self.calls.append(text)

        def text(self) -> str:
            return ""

    callbacks = []
    clipboard = UnavailableClipboard()
    monkeypatch.setattr(QApplication, "clipboard", lambda: clipboard)
    monkeypatch.setattr(
        "github_cli_manager.main_window.QTimer.singleShot",
        lambda _delay, callback: callbacks.append(callback),
    )
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    controller.login_active.emit(True)
    controller.authorization_code.emit("ABCD-EF12")

    window._copy_authorization_code()
    controller.login_active.emit(False)
    callbacks.pop()()

    assert clipboard.calls == ["ABCD-EF12"]


def test_url_copy_retry_is_ignored_immediately_after_cancel(qtbot, monkeypatch) -> None:
    class UnavailableClipboard:
        calls: list[str] = []

        def setText(self, text: str) -> None:
            self.calls.append(text)

        def text(self) -> str:
            return ""

    callbacks = []
    clipboard = UnavailableClipboard()
    monkeypatch.setattr(QApplication, "clipboard", lambda: clipboard)
    monkeypatch.setattr(
        "github_cli_manager.main_window.QTimer.singleShot",
        lambda _delay, callback: callbacks.append(callback),
    )
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    controller.login_active.emit(True)
    controller.authorization_code.emit("ABCD-EF12")

    window._copy_authorization_url()
    qtbot.mouseClick(window.cancel_button, Qt.MouseButton.LeftButton)
    callbacks.pop()()

    assert controller.calls == [("cancel",)]
    assert clipboard.calls == ["https://github.com/login/device"]
    assert callbacks == []


def test_url_copy_retry_is_ignored_after_a_new_login_starts(qtbot, monkeypatch) -> None:
    class UnavailableClipboard:
        calls: list[str] = []

        def setText(self, text: str) -> None:
            self.calls.append(text)

        def text(self) -> str:
            return ""

    callbacks = []
    clipboard = UnavailableClipboard()
    monkeypatch.setattr(QApplication, "clipboard", lambda: clipboard)
    monkeypatch.setattr(
        "github_cli_manager.main_window.QTimer.singleShot",
        lambda _delay, callback: callbacks.append(callback),
    )
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    controller.login_active.emit(True)
    controller.authorization_code.emit("ABCD-EF12")

    window._copy_authorization_url()
    controller.login_active.emit(False)
    controller.login_active.emit(True)
    controller.authorization_code.emit("WXYZ-9876")
    callbacks.pop()()

    assert clipboard.calls == ["https://github.com/login/device"]
    assert callbacks == []


def test_close_shuts_down_controller(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)

    window.close()

    assert controller.calls == [("shutdown",)]


def test_error_message_is_user_friendly(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)

    controller.message_changed.emit("登录失败，请重试", True)

    assert window.status_label.text() == "登录失败，请重试"
    assert window.status_label.property("error") is True


def test_window_exposes_professional_console_structure(qtbot, tmp_path) -> None:
    image = QImage(16, 16, QImage.Format.Format_ARGB32)
    image.fill(QColor("#0969da"))
    icon_path = tmp_path / "githubdeck-icon.png"
    assert image.save(str(icon_path))
    icon = QIcon(str(icon_path))
    window = MainWindow(FakeController(), app_icon=icon)
    qtbot.addWidget(window)

    expected_objects = {
        "mainContainer",
        "appHeader",
        "pageHeader",
        "accountSection",
        "accountCountLabel",
        "bottomActions",
        "statusBar",
        "authorizationTitle",
        "authorizationDescription",
    }

    assert expected_objects <= {
        child.objectName() for child in window.findChildren(QObject)
    }
    assert window.windowTitle() == "GithubDeck"
    assert window.windowIcon().cacheKey() == icon.cacheKey()
    app_title = window.findChild(QLabel, "appTitle")
    app_mark = window.findChild(QLabel, "appMark")
    assert app_title.text() == "GithubDeck"
    assert app_mark.text() == ""
    assert app_mark.pixmap() is not None
    assert not app_mark.pixmap().isNull()
    assert window.account_list.objectName() == "accountList"
    assert window.authorization_widget.objectName() == "authorizationWidget"
    assert window.status_label.objectName() == "statusLabel"


def test_account_cards_preserve_list_semantics_and_visual_state(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    accounts = [
        Account("github.com", "alice", True),
        Account("github.com", "bob", state=AuthState.NEEDS_LOGIN),
        Account("github.com", "carol", state=AuthState.UNKNOWN),
    ]

    controller.accounts_changed.emit(accounts)

    assert window.account_count_label.text() == "3 个账号"
    assert window.account_list.count() == 3
    assert window.account_list.item(0).data(Qt.ItemDataRole.UserRole) == accounts[0]
    assert "alice" in window.account_list.item(0).text()
    assert window.account_list.itemWidget(window.account_list.item(0)).property("active") is True

    window.account_list.setCurrentRow(1)
    selected_card = window.account_list.itemWidget(window.account_list.item(1))
    assert selected_card.property("selected") is True
    assert selected_card.property("authState") == "needs-login"


def test_busy_and_normal_messages_update_visual_properties(qtbot) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)

    controller.busy = True
    controller.busy_changed.emit(True)
    assert window.property("busy") is True

    controller.message_changed.emit("账号状态已刷新", False)
    assert window.status_label.property("error") is False


def test_account_cards_are_opaque_and_keep_stable_geometry(qtbot) -> None:
    window = MainWindow(FakeController())
    qtbot.addWidget(window)
    accounts = [
        Account("github.com", "alice", True),
        Account("github.com", "a-very-long-account-name", False),
    ]

    window.set_accounts(accounts)
    first_item = window.account_list.item(0)
    first_card = window.account_list.itemWidget(first_item)
    second_card = window.account_list.itemWidget(window.account_list.item(1))
    before = first_card.sizeHint().height()

    assert first_card.testAttribute(Qt.WidgetAttribute.WA_StyledBackground)
    assert first_item.foreground().color().alpha() == 0
    assert "alice" in first_item.text()
    assert first_item.data(Qt.ItemDataRole.UserRole) is accounts[0]
    assert first_card.height() == second_card.height()

    window.account_list.setCurrentRow(0)
    assert first_card.sizeHint().height() == before


def test_logout_button_state_and_confirmation(qtbot, monkeypatch) -> None:
    controller = FakeController()
    window = MainWindow(controller)
    qtbot.addWidget(window)
    accounts = [
        Account("github.com", "alice", True),
        Account("github.com", "bob", False),
    ]
    window.set_accounts(accounts)

    assert window.logout_button.objectName() == "logoutButton"
    assert not window.logout_button.isEnabled()

    window.account_list.setCurrentRow(0)
    assert window.logout_button.isEnabled()
    assert not window.switch_button.isEnabled()

    monkeypatch.setattr(window, "_confirm_logout", lambda account: False)
    qtbot.mouseClick(window.logout_button, Qt.MouseButton.LeftButton)
    assert controller.calls == []

    monkeypatch.setattr(window, "_confirm_logout", lambda account: account is accounts[0])
    qtbot.mouseClick(window.logout_button, Qt.MouseButton.LeftButton)
    assert controller.calls == [("logout", accounts[0])]

    controller.busy = True
    controller.busy_changed.emit(True)
    assert not window.logout_button.isEnabled()
