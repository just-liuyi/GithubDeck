from pathlib import Path

from PySide6.QtGui import QColor, QImage

from github_cli_manager.app import configure_application, resource_path


def _write_test_icon(path: Path) -> None:
    image = QImage(16, 16, QImage.Format.Format_ARGB32)
    image.fill(QColor("#0969da"))
    assert image.save(str(path))


def test_application_metadata_and_icon(qapp, tmp_path) -> None:
    icon_path = tmp_path / "githubdeck-icon.png"
    _write_test_icon(icon_path)

    icon = configure_application(qapp, icon_path)

    assert qapp.organizationName() == "VibeDev"
    assert qapp.applicationName() == "GithubDeck"
    assert qapp.applicationDisplayName() == "GithubDeck"
    assert qapp.applicationVersion() == "0.2.0"
    assert not icon.isNull()
    assert not qapp.windowIcon().isNull()


def test_resource_path_targets_source_asset(monkeypatch) -> None:
    monkeypatch.delattr("sys._MEIPASS", raising=False)

    assert resource_path("assets/githubdeck-icon.png") == (
        Path(__file__).resolve().parents[1] / "assets" / "githubdeck-icon.png"
    )


def test_resource_path_supports_pyinstaller_onefile(monkeypatch, tmp_path) -> None:
    monkeypatch.setattr("sys._MEIPASS", str(tmp_path), raising=False)

    assert resource_path("assets/githubdeck-icon.png") == (
        tmp_path / "assets" / "githubdeck-icon.png"
    )
