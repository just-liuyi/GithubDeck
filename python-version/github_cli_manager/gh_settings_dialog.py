import os
from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from .gh_settings import (
    DEFAULT_GH_EXECUTABLE,
    GhExecutableSettings,
    GhExecutableValidator,
)


_RELEASES_URL = "https://github.com/cli/cli/releases/"
_TESTED_MSI = "gh_2.96.0_windows_amd64.msi"


def _refresh_style(widget: QWidget) -> None:
    style = widget.style()
    style.unpolish(widget)
    style.polish(widget)
    widget.update()


class GhSettingsDialog(QDialog):
    def __init__(
        self,
        settings: GhExecutableSettings,
        validator: GhExecutableValidator | None = None,
        parent=None,
        file_dialog=None,
    ) -> None:
        super().__init__(parent)
        self.setWindowTitle("GitHub CLI 设置")
        self.resize(660, 390)
        self.setMinimumSize(580, 360)
        self._settings = settings
        self._validator = validator or GhExecutableValidator(self)
        self._file_dialog = file_dialog or QFileDialog.getOpenFileName
        self._candidate = self._settings.path()
        self._pending_save = False

        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        header = QWidget(self)
        header.setObjectName("ghSettingsHeader")
        header_layout = QVBoxLayout(header)
        header_layout.setContentsMargins(22, 18, 22, 16)
        header_layout.setSpacing(3)
        title = QLabel("GitHub CLI 设置", header)
        title.setObjectName("dialogTitle")
        header_layout.addWidget(title)
        description = QLabel("选择本机 gh.exe，并在保存前完成有效性校验。", header)
        description.setObjectName("dialogDescription")
        header_layout.addWidget(description)
        layout.addWidget(header)

        body = QWidget(self)
        body.setObjectName("dialogContainer")
        body_layout = QVBoxLayout(body)
        body_layout.setContentsMargins(22, 18, 22, 18)
        body_layout.setSpacing(14)

        path_card = QWidget(body)
        path_card.setObjectName("ghSettingsPathCard")
        path_layout = QVBoxLayout(path_card)
        path_layout.setContentsMargins(14, 13, 14, 14)
        path_layout.setSpacing(9)
        path_layout.addWidget(QLabel("GitHub CLI 可执行文件", path_card))
        path_row = QHBoxLayout()
        path_row.setSpacing(8)
        self.path_edit = QLineEdit(self._candidate, path_card)
        self.path_edit.setObjectName("ghPathEdit")
        self.path_edit.setReadOnly(True)
        path_row.addWidget(self.path_edit, 1)
        self.browse_button = QPushButton("选择 gh.exe", path_card)
        self.browse_button.setObjectName("browseButton")
        path_row.addWidget(self.browse_button)
        path_layout.addLayout(path_row)
        reset_row = QHBoxLayout()
        self.reset_button = QPushButton("恢复默认", path_card)
        self.reset_button.setObjectName("resetButton")
        reset_row.addWidget(self.reset_button)
        reset_row.addStretch()
        path_layout.addLayout(reset_row)
        body_layout.addWidget(path_card)

        self.status_label = QLabel("保存前将校验所选 gh.exe。", body)
        self.status_label.setObjectName("ghSettingsStatus")
        self.status_label.setProperty("state", "idle")
        self.status_label.setWordWrap(True)
        body_layout.addWidget(self.status_label)

        help_card = QWidget(body)
        help_card.setObjectName("ghSettingsHelpCard")
        help_layout = QVBoxLayout(help_card)
        help_layout.setContentsMargins(14, 12, 14, 12)
        source = QLabel(help_card)
        source.setObjectName("ghSourceLabel")
        source.setWordWrap(True)
        source.setTextFormat(Qt.TextFormat.RichText)
        source.setOpenExternalLinks(True)
        source.setText(
            f'官方发布页：<a href="{_RELEASES_URL}">{_RELEASES_URL}</a><br>'
            f"已测试安装包：<b>{_TESTED_MSI}</b>。其他合法版本同样可以使用。"
        )
        help_layout.addWidget(source)
        body_layout.addWidget(help_card)
        layout.addWidget(body, 1)

        self.button_box = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Save
            | QDialogButtonBox.StandardButton.Cancel,
            self,
        )
        self.button_box.button(QDialogButtonBox.StandardButton.Save).setText("保存")
        self.button_box.button(QDialogButtonBox.StandardButton.Save).setObjectName("saveButton")
        self.button_box.button(QDialogButtonBox.StandardButton.Cancel).setText("取消")
        button_container = QWidget(self)
        button_container.setObjectName("dialogButtonBar")
        button_layout = QHBoxLayout(button_container)
        button_layout.setContentsMargins(22, 12, 22, 14)
        button_layout.addWidget(self.button_box)
        layout.addWidget(button_container)

        self.browse_button.clicked.connect(self._browse)
        self.reset_button.clicked.connect(self._reset)
        self.button_box.accepted.connect(self._save)
        self.button_box.rejected.connect(self.reject)
        self._validator.finished.connect(self._on_validated)

    def candidate_path(self) -> str:
        return self._candidate

    def reject(self) -> None:
        self._pending_save = False
        cancel = getattr(self._validator, "cancel", None)
        if cancel is not None:
            cancel()
        super().reject()

    def _set_status(self, message: str, state: str) -> None:
        self.status_label.setText(message)
        self.status_label.setProperty("state", state)
        _refresh_style(self.status_label)

    def _set_candidate(self, path: str) -> None:
        self._candidate = os.path.abspath(path) if path else DEFAULT_GH_EXECUTABLE
        self.path_edit.setText(self._candidate)

    def _browse(self) -> None:
        start_dir = str(Path(self._candidate).parent)
        selected, _filter = self._file_dialog(
            self,
            "选择 gh.exe",
            start_dir,
            "可执行文件 (*.exe)",
        )
        if selected:
            self._set_candidate(selected)
            self._set_status("已选择文件，保存前将校验。", "idle")

    def _reset(self) -> None:
        self._set_candidate(DEFAULT_GH_EXECUTABLE)
        self._set_status("已恢复默认路径，保存前将校验。", "idle")

    def _set_controls_enabled(self, enabled: bool) -> None:
        self.browse_button.setEnabled(enabled)
        self.reset_button.setEnabled(enabled)
        self.button_box.button(QDialogButtonBox.StandardButton.Save).setEnabled(enabled)

    def _save(self) -> None:
        if self._validator.busy:
            return
        self._pending_save = True
        self._set_controls_enabled(False)
        self._set_status("正在校验 GitHub CLI…", "validating")
        if not self._validator.validate(self._candidate):
            self._pending_save = False
            self._set_controls_enabled(True)
            self._set_status("当前有校验正在进行，请稍候。", "idle")

    def _on_validated(self, valid: bool, message: str) -> None:
        self._set_controls_enabled(True)
        self._set_status(message, "success" if valid else "error")
        if not self._pending_save:
            return
        self._pending_save = False
        if not valid:
            return
        if self._candidate == DEFAULT_GH_EXECUTABLE:
            self._settings.reset()
        else:
            self._settings.set_path(self._candidate)
        self.accept()
