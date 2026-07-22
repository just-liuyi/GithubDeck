import os
from pathlib import Path
import re

from PySide6.QtCore import (
    QObject,
    QProcess,
    QProcessEnvironment,
    QSettings,
    QTimer,
    Signal,
)


DEFAULT_GH_EXECUTABLE = r"C:\Program Files\GitHub CLI\gh.exe"
_SETTINGS_KEY = "githubCli/executablePath"
_VERSION_TIMEOUT_MS = 5_000
_VERSION_OUTPUT_LIMIT = 4096
_VERSION_RE = re.compile(rb"(?mi)^gh version \d+\.\d+\.\d+(?:[-+._a-z0-9]*)?\b")
_SECRET_ENVIRONMENT_VARIABLES = (
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
)


def sanitized_process_environment() -> QProcessEnvironment:
    environment = QProcessEnvironment.systemEnvironment()
    for variable in _SECRET_ENVIRONMENT_VARIABLES:
        environment.remove(variable)
    environment.remove("GH_BROWSER")
    return environment


class GhExecutableSettings:
    def __init__(self, settings: QSettings | None = None) -> None:
        self._settings = (
            settings if settings is not None else QSettings("VibeDev", "GhDeck")
        )

    def path(self) -> str:
        value = self._settings.value(_SETTINGS_KEY, "", type=str).strip()
        return os.path.abspath(value) if value else DEFAULT_GH_EXECUTABLE

    def set_path(self, executable_path: str) -> None:
        self._settings.setValue(_SETTINGS_KEY, os.path.abspath(executable_path))
        self._settings.sync()

    def reset(self) -> None:
        self._settings.remove(_SETTINGS_KEY)
        self._settings.sync()


class GhExecutableValidator(QObject):
    finished = Signal(bool, str)

    def __init__(self, parent: QObject | None = None) -> None:
        super().__init__(parent)
        self._process = QProcess(self)
        self._process.finished.connect(self._on_finished)
        self._process.errorOccurred.connect(self._on_error)
        self._process.readyReadStandardOutput.connect(self._read_output)
        self._process.readyReadStandardError.connect(self._read_output)
        self._timer = QTimer(self)
        self._timer.setSingleShot(True)
        self._timer.setInterval(_VERSION_TIMEOUT_MS)
        self._timer.timeout.connect(self._on_timeout)
        self._reported = True
        self._timed_out = False
        self._output = b""

    @property
    def busy(self) -> bool:
        return not self._reported

    def validate(self, executable_path: str) -> bool:
        if self.busy:
            return False
        path = Path(executable_path)
        if not path.is_file():
            QTimer.singleShot(0, lambda: self.finished.emit(False, "所选文件不存在"))
            return True
        if path.name.casefold() != "gh.exe":
            QTimer.singleShot(0, lambda: self.finished.emit(False, "请选择名为 gh.exe 的文件"))
            return True

        self._reported = False
        self._timed_out = False
        self._output = b""
        self._process.setProgram(os.path.abspath(executable_path))
        self._process.setArguments(["--version"])
        self._process.setProcessEnvironment(sanitized_process_environment())
        self._process.start()
        self._timer.start()
        return True

    def cancel(self) -> None:
        if not self.busy:
            return
        self._reported = True
        self._timer.stop()
        self._process.kill()
        self._output = b""

    def _read_output(self) -> None:
        data = bytes(self._process.readAllStandardOutput()) + bytes(
            self._process.readAllStandardError()
        )
        if not self._reported:
            self._output = (self._output + data)[-_VERSION_OUTPUT_LIMIT:]

    def _on_finished(self, exit_code: int, _exit_status: QProcess.ExitStatus) -> None:
        if self._reported:
            return
        self._read_output()
        if self._timed_out:
            self._complete(False, "GitHub CLI 版本校验超时")
        elif exit_code != 0:
            self._complete(False, "无法执行所选 GitHub CLI")
        elif _VERSION_RE.search(self._output) is None:
            self._complete(False, "所选文件不是有效的 GitHub CLI")
        else:
            self._complete(True, "GitHub CLI 校验通过")

    def _on_error(self, error: QProcess.ProcessError) -> None:
        if error == QProcess.ProcessError.FailedToStart and not self._reported:
            self._complete(False, "无法启动所选 GitHub CLI")

    def _on_timeout(self) -> None:
        if self._reported:
            return
        self._timed_out = True
        self._process.kill()
        if self._process.state() == QProcess.ProcessState.NotRunning:
            self._complete(False, "GitHub CLI 版本校验超时")

    def _complete(self, valid: bool, message: str) -> None:
        if self._reported:
            return
        self._reported = True
        self._timer.stop()
        self._output = b""
        self.finished.emit(valid, message)
