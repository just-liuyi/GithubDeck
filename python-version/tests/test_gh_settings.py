import os

from PySide6.QtCore import QCoreApplication, QProcess, QSettings

from github_cli_manager.gh_settings import (
    DEFAULT_GH_EXECUTABLE,
    GhExecutableSettings,
    GhExecutableValidator,
)


def test_default_settings_keep_legacy_identity_when_app_name_changes(qapp, tmp_path) -> None:
    original_format = QSettings.defaultFormat()
    original_path = QSettings().fileName()
    original_app_name = QCoreApplication.applicationName()
    QSettings.setDefaultFormat(QSettings.Format.IniFormat)
    QSettings.setPath(QSettings.Format.IniFormat, QSettings.Scope.UserScope, str(tmp_path))
    QCoreApplication.setApplicationName("GithubDeck")
    try:
        settings = GhExecutableSettings()
        custom = tmp_path / "legacy" / "gh.exe"
        settings.set_path(str(custom))

        legacy = QSettings("VibeDev", "GhDeck")
        visible_identity = QSettings("VibeDev", "GithubDeck")
        assert legacy.value("githubCli/executablePath", "", type=str) == os.path.abspath(custom)
        assert not visible_identity.contains("githubCli/executablePath")
        assert settings.path() == os.path.abspath(custom)
    finally:
        QCoreApplication.setApplicationName(original_app_name)
        QSettings.setDefaultFormat(original_format)
        QSettings.setPath(original_format, QSettings.Scope.UserScope, str(os.path.dirname(original_path)))


def test_settings_uses_default_when_key_is_missing(tmp_path) -> None:
    qsettings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)

    assert GhExecutableSettings(qsettings).path() == DEFAULT_GH_EXECUTABLE


def test_settings_persists_normalized_custom_path(tmp_path) -> None:
    qsettings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    settings = GhExecutableSettings(qsettings)
    custom = tmp_path / "bin" / "gh.exe"

    settings.set_path(str(custom))

    reloaded = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    assert GhExecutableSettings(reloaded).path() == os.path.abspath(custom)


def test_reset_removes_custom_path(tmp_path) -> None:
    qsettings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    settings = GhExecutableSettings(qsettings)
    settings.set_path(str(tmp_path / "gh.exe"))

    settings.reset()

    assert settings.path() == DEFAULT_GH_EXECUTABLE
    assert not qsettings.contains("githubCli/executablePath")


def test_invalid_saved_path_is_preserved(tmp_path) -> None:
    qsettings = QSettings(str(tmp_path / "settings.ini"), QSettings.Format.IniFormat)
    settings = GhExecutableSettings(qsettings)
    missing = tmp_path / "missing" / "gh.exe"

    settings.set_path(str(missing))

    assert settings.path() == os.path.abspath(missing)


def test_validator_rejects_missing_and_wrong_named_files(qtbot, tmp_path) -> None:
    validator = GhExecutableValidator()
    wrong = tmp_path / "tool.exe"
    wrong.write_bytes(b"")

    with qtbot.waitSignal(validator.finished) as missing_signal:
        assert validator.validate(str(tmp_path / "gh.exe"))
    assert missing_signal.args == [False, "所选文件不存在"]

    with qtbot.waitSignal(validator.finished) as wrong_signal:
        assert validator.validate(str(wrong))
    assert wrong_signal.args == [False, "请选择名为 gh.exe 的文件"]


def test_validator_accepts_github_cli_version_output(monkeypatch, tmp_path) -> None:
    executable = tmp_path / "gh.exe"
    executable.write_bytes(b"")
    validator = GhExecutableValidator()
    results: list[tuple[bool, str]] = []
    validator.finished.connect(lambda valid, message: results.append((valid, message)))
    monkeypatch.setattr(validator._process, "start", lambda: None)
    monkeypatch.setattr(
        validator._process,
        "readAllStandardOutput",
        lambda: b"gh version 2.97.1 (2026-07-01)\n",
    )
    monkeypatch.setattr(validator._process, "readAllStandardError", lambda: b"")

    assert validator.validate(str(executable))
    validator._on_finished(0, QProcess.ExitStatus.NormalExit)

    assert results == [(True, "GitHub CLI 校验通过")]


def test_validator_sanitizes_candidate_process_environment(monkeypatch, tmp_path) -> None:
    executable = tmp_path / "gh.exe"
    executable.write_bytes(b"")
    validator = GhExecutableValidator()
    monkeypatch.setattr(validator._process, "start", lambda: None)
    for name in (
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "GH_ENTERPRISE_TOKEN",
        "GITHUB_ENTERPRISE_TOKEN",
        "GH_BROWSER",
    ):
        monkeypatch.setenv(name, "secret")

    assert validator.validate(str(executable))
    environment = validator._process.processEnvironment()

    assert all(not environment.contains(name) for name in (
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "GH_ENTERPRISE_TOKEN",
        "GITHUB_ENTERPRISE_TOKEN",
        "GH_BROWSER",
    ))


def test_validator_rejects_non_cli_output_without_echoing_it(monkeypatch, tmp_path) -> None:
    executable = tmp_path / "gh.exe"
    executable.write_bytes(b"")
    validator = GhExecutableValidator()
    results: list[tuple[bool, str]] = []
    validator.finished.connect(lambda valid, message: results.append((valid, message)))
    monkeypatch.setattr(validator._process, "start", lambda: None)
    monkeypatch.setattr(
        validator._process,
        "readAllStandardOutput",
        lambda: b"secret untrusted output",
    )
    monkeypatch.setattr(validator._process, "readAllStandardError", lambda: b"")

    assert validator.validate(str(executable))
    validator._on_finished(0, QProcess.ExitStatus.NormalExit)

    assert results == [(False, "所选文件不是有效的 GitHub CLI")]
    assert "secret" not in results[0][1]
