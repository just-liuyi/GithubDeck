from PySide6.QtCore import QObject, Signal

from github_cli_manager.controller import AuthController
from github_cli_manager.models import Account


class FakeProcess(QObject):
    finished = Signal(str, bool, bytes)
    authorization_code = Signal(str)

    def __init__(self) -> None:
        super().__init__()
        self.calls: list[tuple] = []
        self.executable_available = True

    def refresh(self) -> bool:
        self.calls.append(("refresh",))
        return True

    def switch(self, host: str, login: str) -> bool:
        self.calls.append(("switch", host, login))
        return True

    def login(self) -> bool:
        self.calls.append(("login",))
        return True

    def logout(self, host: str, login: str) -> bool:
        self.calls.append(("logout", host, login))
        return True

    def cancel(self) -> bool:
        self.calls.append(("cancel",))
        return True

    def shutdown(self) -> None:
        self.calls.append(("shutdown",))


def test_refresh_parses_and_emits_accounts(qtbot) -> None:
    process = FakeProcess()
    controller = AuthController(process)

    with qtbot.waitSignal(controller.accounts_changed) as signal:
        assert controller.refresh()
        process.finished.emit(
            "status", True, b'{"hosts":{"github.com":[{"login":"alice","active":true}]}}'
        )

    assert signal.args == [[Account("github.com", "alice", True)]]
    assert not controller.busy


def test_switch_is_serial_and_refreshes_for_confirmation(qtbot) -> None:
    process = FakeProcess()
    controller = AuthController(process)
    emitted_accounts: list[Account] = []
    controller.accounts_changed.connect(emitted_accounts.extend)
    assert controller.refresh()
    process.finished.emit(
        "status", True, b'{"hosts":{"github.com":[{"login":"bob","active":false}]}}'
    )
    account = emitted_accounts[0]
    process.calls.clear()

    assert controller.switch_account(account)
    assert not controller.login()
    assert process.calls == [("switch", "github.com", "bob")]

    process.finished.emit("switch", True, b"ignored output")
    assert process.calls[-1] == ("refresh",)
    assert controller.busy

    with qtbot.waitSignal(controller.message_changed) as signal:
        process.finished.emit(
            "status", True, b'{"hosts":{"github.com":[{"login":"bob","active":true}]}}'
        )
    assert signal.args == ["账号切换成功", False]
    assert not controller.busy


def test_logout_is_serial_and_refreshes_for_confirmation(qtbot) -> None:
    process = FakeProcess()
    controller = AuthController(process)
    emitted_accounts: list[Account] = []
    controller.accounts_changed.connect(emitted_accounts.extend)
    controller.refresh()
    process.finished.emit(
        "status", True, b'{"hosts":{"github.com":[{"login":"alice","active":true}]}}'
    )
    account = emitted_accounts[0]
    process.calls.clear()

    assert controller.logout(account)
    assert process.calls == [("logout", "github.com", "alice")]
    assert controller.busy

    process.finished.emit("logout", True, b"ignored")
    assert process.calls[-1] == ("refresh",)

    with qtbot.waitSignal(controller.message_changed) as signal:
        process.finished.emit("status", True, b'{"hosts":{}}')
    assert signal.args == ["账号登出成功", False]
    assert not controller.busy


def test_logout_rejects_untrusted_account_and_handles_failure(qtbot) -> None:
    process = FakeProcess()
    controller = AuthController(process)
    emitted: list[list[Account]] = []
    controller.accounts_changed.connect(emitted.append)
    controller.refresh()
    process.finished.emit(
        "status", True, b'{"hosts":{"github.com":[{"login":"alice"}]}}'
    )
    trusted = emitted[-1][0]
    process.calls.clear()

    assert not controller.logout(Account("github.com", "alice"))
    assert process.calls == []

    messages: list[tuple[str, bool]] = []
    controller.message_changed.connect(
        lambda message, is_error: messages.append((message, is_error))
    )
    assert controller.logout(trusted)
    process.finished.emit("logout", False, b"secret output")

    assert messages[-1] == ("登出账号失败，请检查 GitHub CLI 配置后重试", True)
    assert not controller.busy
    assert emitted[-1] == [trusted]


def test_login_uses_process_then_refreshes() -> None:
    process = FakeProcess()
    controller = AuthController(process)

    assert controller.login()
    process.finished.emit("login", True, b"")

    assert process.calls == [("login",), ("refresh",)]


def test_login_code_and_active_lifecycle() -> None:
    process = FakeProcess()
    controller = AuthController(process)
    codes: list[str] = []
    active: list[bool] = []
    controller.authorization_code.connect(codes.append)
    controller.login_active.connect(active.append)

    assert controller.login()
    process.authorization_code.emit("ABCD-EF12")
    assert codes[-1] == "ABCD-EF12"
    assert active == [True]
    assert not controller.refresh()

    process.finished.emit("login", False, b"")
    assert codes[-1] == ""
    assert active == [True, False]
    assert not controller.busy


def test_cancel_only_works_for_login() -> None:
    process = FakeProcess()
    controller = AuthController(process)

    assert controller.refresh()
    assert not controller.cancel()
    assert process.calls == [("refresh",)]


def test_nonzero_status_with_valid_json_emits_accounts_and_warning(qtbot) -> None:
    process = FakeProcess()
    controller = AuthController(process)

    messages: list[tuple[str, bool]] = []
    controller.message_changed.connect(lambda text, error: messages.append((text, error)))
    with qtbot.waitSignal(controller.accounts_changed) as accounts_signal:
        assert controller.refresh()
        process.finished.emit(
            "status", False, b'{"hosts":{"github.com":[{"login":"alice"}]}}'
        )

    assert accounts_signal.args == [[Account("github.com", "alice")]]
    assert messages[-1] == ("已显示可用账号，部分账号可能需要重新认证", True)


def test_parse_failure_clears_stale_accounts() -> None:
    process = FakeProcess()
    controller = AuthController(process)
    emitted: list[list[Account]] = []
    controller.accounts_changed.connect(emitted.append)
    controller.refresh()
    process.finished.emit(
        "status", True, b'{"hosts":{"github.com":[{"login":"alice"}]}}'
    )
    stale = emitted[-1][0]

    controller.refresh()
    process.finished.emit("status", False, b"invalid")

    assert emitted[-1] == []
    assert not controller.switch_account(stale)


def test_switch_rejects_equal_but_untrusted_account() -> None:
    process = FakeProcess()
    controller = AuthController(process)
    controller.refresh()
    process.finished.emit(
        "status", True, b'{"hosts":{"github.com":[{"login":"alice"}]}}'
    )

    assert not controller.switch_account(Account("github.com", "alice"))
    assert process.calls == [("refresh",)]


def test_cancel_and_shutdown_are_delegated() -> None:
    process = FakeProcess()
    controller = AuthController(process)
    active: list[bool] = []
    controller.login_active.connect(active.append)

    assert controller.login()
    assert controller.cancel()
    assert active == [True, False]
    assert controller.busy
    controller.shutdown()

    assert process.calls == [("login",), ("cancel",), ("shutdown",)]


def test_missing_executable_is_rejected_before_start(qtbot) -> None:
    process = FakeProcess()
    process.executable_available = False
    controller = AuthController(process)

    with qtbot.waitSignal(controller.message_changed) as signal:
        assert not controller.refresh()

    assert signal.args == ["未找到 GitHub CLI，请在设置中指定有效的 gh.exe", True]
    assert process.calls == []
