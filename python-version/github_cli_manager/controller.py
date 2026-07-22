from PySide6.QtCore import QObject, Signal

from .auth_parser import AuthStatusError, parse_auth_status
from .gh_process import GhProcess
from .models import Account


class AuthController(QObject):
    accounts_changed = Signal(list)
    busy_changed = Signal(bool)
    message_changed = Signal(str, bool)
    authorization_code = Signal(str)
    login_active = Signal(bool)

    def __init__(self, process: GhProcess | None = None, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._process = process or GhProcess(parent=self)
        self._process.finished.connect(self._on_process_finished)
        process_code_signal = getattr(self._process, "authorization_code", None)
        if process_code_signal is not None:
            process_code_signal.connect(self._on_authorization_code)
        self._busy = False
        self._login_active = False
        self._confirmation = ""
        self._accounts_snapshot: tuple[Account, ...] = ()
        self._cancel_requested = False

    @property
    def busy(self) -> bool:
        return self._busy

    @property
    def gh_settings(self):
        return getattr(self._process, "settings", None)

    def refresh(self) -> bool:
        if self._busy:
            return False
        if not self._ensure_executable():
            return False
        self._set_busy(True)
        self.message_changed.emit("正在刷新账号…", False)
        if not self._process.refresh():
            self._set_busy(False)
            self.message_changed.emit("当前有操作正在进行", True)
            return False
        return True

    def switch_account(self, account: Account) -> bool:
        if self._busy:
            return False
        if self._trusted_snapshot_account(account) is None:
            self.message_changed.emit("账号列表已变化，请刷新后重试", True)
            return False
        if not self._ensure_executable():
            return False
        self._set_busy(True)
        self.message_changed.emit(f"正在切换到 {account.login}…", False)
        if not self._process.switch(account.host, account.login):
            self._set_busy(False)
            self.message_changed.emit("当前有操作正在进行", True)
            return False
        return True

    def logout(self, account: Account) -> bool:
        if self._busy:
            return False
        if self._trusted_snapshot_account(account) is None:
            self.message_changed.emit("账号列表已变化，请刷新后重试", True)
            return False
        if not self._ensure_executable():
            return False
        self._set_busy(True)
        self.message_changed.emit(f"正在登出 {account.login}…", False)
        if not self._process.logout(account.host, account.login):
            self._set_busy(False)
            self.message_changed.emit("当前有操作正在进行", True)
            return False
        return True

    def login(self) -> bool:
        if self._busy:
            return False
        if not self._ensure_executable():
            return False
        self._cancel_requested = False
        self.authorization_code.emit("")
        self._set_busy(True)
        self._set_login_active(True)
        self.message_changed.emit(
            "正在生成授权码并打开 GitHub 官方授权页面…",
            False,
        )
        if not self._process.login():
            self._set_login_active(False)
            self._set_busy(False)
            self.message_changed.emit("当前有操作正在进行", True)
            return False
        return True

    def cancel(self) -> bool:
        if not self._login_active or not self._process.cancel():
            return False
        self._cancel_requested = True
        self.authorization_code.emit("")
        self._set_login_active(False)
        self.message_changed.emit("正在取消登录…", False)
        return True

    def shutdown(self) -> None:
        self._process.shutdown()

    def _trusted_snapshot_account(self, account: Account) -> Account | None:
        matched = next(
            (known for known in self._accounts_snapshot if known is account),
            None,
        )
        return matched if matched == account else None

    def _ensure_executable(self) -> bool:
        available = getattr(self._process, "executable_available", True)
        if available:
            return True
        self.message_changed.emit("未找到 GitHub CLI，请在设置中指定有效的 gh.exe", True)
        return False

    def _set_busy(self, busy: bool) -> None:
        if self._busy != busy:
            self._busy = busy
            self.busy_changed.emit(busy)

    def _set_login_active(self, active: bool) -> None:
        if self._login_active != active:
            self._login_active = active
            self.login_active.emit(active)

    def _on_authorization_code(self, code: str) -> None:
        if self._login_active:
            self.authorization_code.emit(code)

    def _on_process_finished(self, operation: str, success: bool, output: bytes) -> None:
        if operation == "status":
            self._finish_refresh(success, output)
            return

        if operation == "login":
            self.authorization_code.emit("")
            self._set_login_active(False)

        if not success:
            self._set_busy(False)
            if operation == "login" and self._cancel_requested:
                self._cancel_requested = False
                self.message_changed.emit("登录已取消", False)
                return
            actions = {
                "switch": "切换账号",
                "logout": "登出账号",
                "login": "登录",
            }
            action = actions.get(operation, "操作")
            self.message_changed.emit(f"{action}失败，请检查 GitHub CLI 配置后重试", True)
            return

        self._cancel_requested = False
        confirmations = {
            "switch": "账号切换成功",
            "logout": "账号登出成功",
            "login": "登录成功",
        }
        self._confirmation = confirmations.get(operation, "操作成功")
        self.message_changed.emit("操作完成，正在刷新确认…", False)
        if not self._process.refresh():
            self._confirmation = ""
            self._set_busy(False)
            self.message_changed.emit("操作完成，但无法刷新确认", True)

    def _finish_refresh(self, success: bool, output: bytes) -> None:
        try:
            accounts = parse_auth_status(output)
        except AuthStatusError as exc:
            self._confirmation = ""
            self._accounts_snapshot = ()
            self.accounts_changed.emit([])
            self._set_busy(False)
            self.message_changed.emit(str(exc), True)
            return

        self._accounts_snapshot = tuple(accounts)
        self.accounts_changed.emit(accounts)
        if not success and accounts:
            message = "已显示可用账号，部分账号可能需要重新认证"
            is_error = True
        else:
            message = self._confirmation or ("账号列表已刷新" if accounts else "尚未登录 GitHub 账号")
            is_error = False
        self._confirmation = ""
        self._set_busy(False)
        self.message_changed.emit(message, is_error)
