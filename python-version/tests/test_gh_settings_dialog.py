from PySide6.QtCore import QObject, QSettings, Qt, Signal
from PySide6.QtWidgets import QDialogButtonBox

from github_cli_manager.gh_settings import DEFAULT_GH_EXECUTABLE, GhExecutableSettings
from github_cli_manager.gh_settings_dialog import GhSettingsDialog


class FakeValidator(QObject):
    finished = Signal(bool, str)

    def __init__(self) -> None:
        super().__init__()
        self.busy = False
        self.paths: list[str] = []

    def validate(self, path: str) -> bool:
        self.paths.append(path)
        self.busy = True
        return True

    def cancel(self) -> None:
        self.busy = False

    def complete(self, valid: bool, message: str) -> None:
        self.busy = False
        self.finished.emit(valid, message)


def make_settings(tmp_path) -> GhExecutableSettings:
    qsettings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    return GhExecutableSettings(qsettings)


def test_dialog_selects_validates_and_saves_custom_path(qtbot, tmp_path) -> None:
    settings = make_settings(tmp_path)
    validator = FakeValidator()
    selected = str(tmp_path / "gh.exe")
    dialog = GhSettingsDialog(
        settings,
        validator,
        file_dialog=lambda *_args: (selected, "可执行文件 (*.exe)"),
    )
    qtbot.addWidget(dialog)

    dialog._browse()
    save = dialog.button_box.button(QDialogButtonBox.StandardButton.Save)
    save.click()

    assert validator.paths == [selected]
    assert not save.isEnabled()
    assert dialog.button_box.button(QDialogButtonBox.StandardButton.Cancel).isEnabled()
    validator.complete(True, "GitHub CLI 校验通过")

    assert settings.path() == selected
    assert dialog.result() == dialog.DialogCode.Accepted


def test_cancelled_file_picker_keeps_candidate(qtbot, tmp_path) -> None:
    settings = make_settings(tmp_path)
    custom = str(tmp_path / "current" / "gh.exe")
    settings.set_path(custom)
    dialog = GhSettingsDialog(
        settings,
        FakeValidator(),
        file_dialog=lambda *_args: ("", ""),
    )
    qtbot.addWidget(dialog)

    dialog._browse()

    assert dialog.candidate_path() == custom


def test_restore_default_removes_setting_after_validation(qtbot, tmp_path) -> None:
    settings = make_settings(tmp_path)
    settings.set_path(str(tmp_path / "custom" / "gh.exe"))
    validator = FakeValidator()
    dialog = GhSettingsDialog(settings, validator)
    qtbot.addWidget(dialog)

    dialog._reset()
    dialog._save()
    validator.complete(True, "GitHub CLI 校验通过")

    assert settings.path() == DEFAULT_GH_EXECUTABLE


def test_failed_validation_keeps_dialog_open_and_setting_unchanged(qtbot, tmp_path) -> None:
    settings = make_settings(tmp_path)
    original = str(tmp_path / "original" / "gh.exe")
    selected = str(tmp_path / "other" / "gh.exe")
    settings.set_path(original)
    validator = FakeValidator()
    dialog = GhSettingsDialog(
        settings,
        validator,
        file_dialog=lambda *_args: (selected, ""),
    )
    qtbot.addWidget(dialog)

    dialog._browse()
    dialog._save()
    validator.complete(False, "所选文件不是有效的 GitHub CLI")

    assert settings.path() == original
    assert dialog.result() == dialog.DialogCode.Rejected
    assert dialog.status_label.text() == "所选文件不是有效的 GitHub CLI"
    assert dialog.browse_button.isEnabled()


def test_reject_during_validation_prevents_late_save(qtbot, tmp_path) -> None:
    settings = make_settings(tmp_path)
    original = str(tmp_path / "original" / "gh.exe")
    selected = str(tmp_path / "selected" / "gh.exe")
    settings.set_path(original)
    validator = FakeValidator()
    dialog = GhSettingsDialog(
        settings,
        validator,
        file_dialog=lambda *_args: (selected, ""),
    )
    qtbot.addWidget(dialog)

    dialog._browse()
    dialog._save()
    dialog.reject()
    validator.complete(True, "GitHub CLI 校验通过")

    assert settings.path() == original
    assert dialog.result() == dialog.DialogCode.Rejected


def test_dialog_declares_official_source_and_tested_msi(qtbot, tmp_path) -> None:
    dialog = GhSettingsDialog(make_settings(tmp_path), FakeValidator())
    qtbot.addWidget(dialog)
    source = dialog.findChild(type(dialog.status_label), "ghSourceLabel")

    assert source.textFormat() == Qt.TextFormat.RichText
    assert source.openExternalLinks()
    assert source.text().count("https://github.com/cli/cli/releases/") == 2
    assert source.text().startswith("官方发布页：")
    assert "gh_2.96.0_windows_amd64.msi" in source.text()


def test_dialog_exposes_professional_console_structure(qtbot, tmp_path) -> None:
    dialog = GhSettingsDialog(make_settings(tmp_path), FakeValidator())
    qtbot.addWidget(dialog)

    expected_objects = {
        "ghSettingsHeader",
        "ghSettingsPathCard",
        "ghSettingsHelpCard",
    }

    assert expected_objects <= {
        child.objectName() for child in dialog.findChildren(QObject)
    }
    assert dialog.path_edit.objectName() == "ghPathEdit"
    assert dialog.status_label.property("state") == "idle"


def test_dialog_tracks_validation_visual_state(qtbot, tmp_path) -> None:
    validator = FakeValidator()
    dialog = GhSettingsDialog(make_settings(tmp_path), validator)
    qtbot.addWidget(dialog)

    dialog._save()

    save = dialog.button_box.button(QDialogButtonBox.StandardButton.Save)
    cancel = dialog.button_box.button(QDialogButtonBox.StandardButton.Cancel)
    assert dialog.status_label.property("state") == "validating"
    assert not save.isEnabled()
    assert cancel.isEnabled()

    validator.complete(False, "所选文件不是有效的 GitHub CLI")
    assert dialog.status_label.property("state") == "error"
