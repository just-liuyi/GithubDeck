from pathlib import Path
import re

from PySide6.QtCore import QObject, QProcess, QTimer, Signal

from .gh_settings import (
    DEFAULT_GH_EXECUTABLE,
    GhExecutableSettings,
    sanitized_process_environment,
)


_STATUS_TIMEOUT_MS = 15_000
_SWITCH_TIMEOUT_MS = 30_000
_LOGOUT_TIMEOUT_MS = 30_000
_ANSI_ESCAPE_RE = re.compile(rb"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))")
_DEVICE_CODE_RE = re.compile(rb"(?<![A-Z0-9-])([A-Z0-9]{4}-[A-Z0-9]{4})(?![A-Z0-9-])")
_LOGIN_BUFFER_LIMIT = 4096
_STATUS_OUTPUT_LIMIT = 1_048_576


class GhProcess(QObject):
    finished = Signal(str, bool, bytes)
    authorization_code = Signal(str)

    def __init__(
        self,
        settings: GhExecutableSettings | None = None,
        parent: QObject | None = None,
    ) -> None:
        super().__init__(parent)
        self._settings = settings or GhExecutableSettings()
        self._process = QProcess(self)
        self._process.finished.connect(self._on_finished)
        self._process.errorOccurred.connect(self._on_error)
        self._process.readyReadStandardOutput.connect(self._read_output)
        self._process.readyReadStandardError.connect(self._read_error)
        self._operation = ""
        self._reported = False
        self._cancelled = False
        self._login_buffer = b""
        self._status_output = b""
        self._code_reported = False
        self._browser_open_confirmed = False
        self._kill_timer = QTimer(self)
        self._kill_timer.setSingleShot(True)
        self._kill_timer.setInterval(2000)
        self._kill_timer.timeout.connect(self._kill_if_running)
        self._operation_timer = QTimer(self)
        self._operation_timer.setSingleShot(True)
        self._operation_timer.timeout.connect(self._on_operation_timeout)

    @property
    def busy(self) -> bool:
        return self._process.state() != QProcess.ProcessState.NotRunning

    @property
    def executable_available(self) -> bool:
        return Path(self._settings.path()).is_file()

    @property
    def settings(self) -> GhExecutableSettings:
        return self._settings

    def executable_path(self) -> str:
        return self._settings.path()

    def refresh(self) -> bool:
        return self._start("status", ["auth", "status", "--json", "hosts"])

    def switch(self, host: str, login: str) -> bool:
        return self._start(
            "switch", ["auth", "switch", "--hostname", host, "--user", login]
        )

    def logout(self, host: str, login: str) -> bool:
        return self._start(
            "logout",
            ["auth", "logout", "--hostname", host, "--user", login],
        )

    def login(self) -> bool:
        return self._start(
            "login",
            [
                "auth",
                "login",
                "--hostname",
                "github.com",
                "--git-protocol",
                "https",
                "--web",
            ],
        )

    def cancel(self) -> bool:
        if not self.busy:
            return False
        self._cancelled = True
        self._clear_login_state()
        self._process.terminate()
        self._kill_timer.start()
        return True

    def shutdown(self) -> None:
        if not self.busy:
            return
        self._cancelled = True
        self._clear_login_state()
        self._operation_timer.stop()
        self._kill_timer.stop()
        self._process.terminate()
        if not self._process.waitForFinished(250):
            self._process.kill()
            self._process.waitForFinished(250)

    def _start(self, operation: str, arguments: list[str]) -> bool:
        if self.busy:
            return False
        executable = self._settings.path() or DEFAULT_GH_EXECUTABLE
        self._process.setProcessEnvironment(sanitized_process_environment())
        self._clear_login_state()
        self._operation = operation
        self._reported = False
        self._cancelled = False
        self._process.setProgram(executable)
        self._process.setArguments(arguments)
        self._process.start()
        if operation == "status":
            self._operation_timer.start(_STATUS_TIMEOUT_MS)
        elif operation == "switch":
            self._operation_timer.start(_SWITCH_TIMEOUT_MS)
        elif operation == "logout":
            self._operation_timer.start(_LOGOUT_TIMEOUT_MS)
        return True

    def _read_output(self) -> None:
        data = bytes(self._process.readAllStandardOutput())
        if self._operation == "login":
            self._consume_login_output(data)
        elif self._operation == "status":
            self._status_output = (self._status_output + data)[-_STATUS_OUTPUT_LIMIT:]

    def _read_error(self) -> None:
        data = bytes(self._process.readAllStandardError())
        if self._operation == "login":
            self._consume_login_output(data)

    def _consume_login_output(self, data: bytes) -> None:
        if self._cancelled or self._code_reported or not data:
            return
        self._login_buffer = (self._login_buffer + data)[-_LOGIN_BUFFER_LIMIT:]
        clean = _ANSI_ESCAPE_RE.sub(b"", self._login_buffer)
        match = _DEVICE_CODE_RE.search(clean)
        if match is not None:
            self._code_reported = True
            self.authorization_code.emit(match.group(1).decode("ascii"))
            self._confirm_browser_open()

    def _confirm_browser_open(self) -> None:
        if self._browser_open_confirmed or self._cancelled or self._operation != "login":
            return
        self._browser_open_confirmed = True
        self._process.write(b"\n")

    def _clear_login_state(self) -> None:
        self._login_buffer = b""
        self._status_output = b""
        self._code_reported = False
        self._browser_open_confirmed = False

    def _on_finished(self, exit_code: int, _exit_status: QProcess.ExitStatus) -> None:
        self._operation_timer.stop()
        self._kill_timer.stop()
        if self._reported:
            return
        self._read_output()
        self._read_error()
        self._reported = True
        output = self._status_output if self._operation == "status" else b""
        operation = self._operation
        success = exit_code == 0 and not self._cancelled
        self._clear_login_state()
        self.finished.emit(operation, success, output)

    def _on_error(self, error: QProcess.ProcessError) -> None:
        if error != QProcess.ProcessError.FailedToStart or self._reported:
            return
        self._operation_timer.stop()
        self._kill_timer.stop()
        self._reported = True
        operation = self._operation
        self._clear_login_state()
        self.finished.emit(operation, False, b"")

    def _on_operation_timeout(self) -> None:
        if self.busy:
            self.cancel()

    def _kill_if_running(self) -> None:
        if self.busy:
            self._process.kill()
