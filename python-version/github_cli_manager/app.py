import sys
from pathlib import Path

from PySide6.QtGui import QIcon
from PySide6.QtWidgets import QApplication

from .controller import AuthController
from .gh_process import GhProcess
from .gh_settings import GhExecutableSettings
from .main_window import MainWindow


GITHUB_CONSOLE_STYLE_SHEET = """
QWidget {
    color: #1f2328;
    font-family: "Segoe UI", "Microsoft YaHei UI", sans-serif;
    font-size: 13px;
}
QMainWindow, QDialog, QWidget#mainContainer, QWidget#dialogContainer {
    background: #f6f8fa;
}
QWidget#appHeader {
    background: #24292f;
    border-bottom: 1px solid #1b1f23;
}
QLabel#appTitle {
    color: #f0f6fc;
    font-weight: 600;
}
QLabel#appMark {
    background: transparent;
    border: none;
}
QLabel#pageTitle, QLabel#dialogTitle {
    color: #1f2328;
    font-size: 22px;
    font-weight: 700;
}
QLabel#pageDescription, QLabel#dialogDescription, QLabel#accountSectionLabel,
QLabel#accountCountLabel, QLabel#authorizationDescription, QLabel#statusVersion {
    color: #656d76;
}
QPushButton {
    min-height: 30px;
    padding: 0 12px;
    background: #ffffff;
    border: 1px solid #d0d7de;
    border-radius: 6px;
    font-weight: 600;
}
QPushButton:hover {
    background: #f3f4f6;
    border-color: #afb8c1;
}
QPushButton:pressed { background: #eaeef2; }
QPushButton:disabled {
    color: #8c959f;
    background: #f6f8fa;
    border-color: #d8dee4;
}
QPushButton#loginButton, QPushButton#saveButton {
    color: #ffffff;
    background: #1f883d;
    border-color: #1a7f37;
}
QPushButton#loginButton:hover, QPushButton#saveButton:hover { background: #1a7f37; }
QPushButton#cancelButton, QPushButton#logoutButton { color: #cf222e; }
QPushButton#logoutButton:hover {
    color: #cf222e;
    background: #fff8f8;
    border-color: #cf222e;
}
QPushButton#logoutButton:disabled {
    color: #8c959f;
    background: #f6f8fa;
    border-color: #d8dee4;
}
QWidget#accountSection, QWidget#ghSettingsPathCard, QWidget#ghSettingsHelpCard {
    background: #ffffff;
    border: 1px solid #d0d7de;
    border-radius: 6px;
}
QListWidget#accountList {
    background: transparent;
    border: none;
    outline: none;
}
QListWidget#accountList::item {
    color: transparent;
    background: transparent;
    border: none;
    padding: 0;
}
QListWidget#accountList::item:selected {
    color: transparent;
    background: transparent;
}
QWidget#accountCard {
    background: #ffffff;
    border: 2px solid #d0d7de;
    border-radius: 6px;
}
QWidget#accountCard:hover { border-color: #afb8c1; }
QWidget#accountCard[selected="true"] { border-color: #0969da; }
QWidget#accountAccent {
    background: transparent;
    border-radius: 2px;
}
QWidget#accountAccent[active="true"] { background: #1f883d; }
QLabel#accountAvatar {
    min-width: 38px;
    min-height: 38px;
    max-width: 38px;
    max-height: 38px;
    color: #ffffff;
    background: #24292f;
    border-radius: 19px;
    font-weight: 700;
}
QLabel#accountLogin { font-weight: 700; }
QLabel#accountMeta { color: #656d76; }
QLabel#accountBadge {
    padding: 2px 8px;
    color: #57606a;
    background: #f6f8fa;
    border: 1px solid #d0d7de;
    border-radius: 10px;
    font-size: 11px;
}
QLabel#accountBadge[active="true"] {
    color: #116329;
    background: #dafbe1;
    border-color: #aceebb;
}
QLabel#accountBadge[authState="needs-login"] {
    color: #633c01;
    background: #fff8c5;
    border-color: #d4a72c;
}
QWidget#authorizationWidget {
    background: #ddf4ff;
    border: 1px solid #54aeff;
    border-radius: 6px;
}
QLabel#authorizationTitle { font-weight: 700; }
QLineEdit {
    min-height: 30px;
    padding: 0 9px;
    color: #1f2328;
    background: #ffffff;
    border: 1px solid #d0d7de;
    border-radius: 6px;
    selection-background-color: #0969da;
}
QLineEdit:read-only { background: #f6f8fa; color: #57606a; }
QLineEdit#authorizationCode { font-family: "Cascadia Mono", Consolas, monospace; font-weight: 700; }
QWidget#statusBar {
    background: #ffffff;
    border-top: 1px solid #d0d7de;
}
QLabel#statusDot { color: #1f883d; }
QLabel#statusDot[error="true"], QLabel#statusLabel[error="true"] { color: #cf222e; }
QWidget#ghSettingsHeader { border-bottom: 1px solid #d0d7de; }
QLabel#ghSettingsStatus[state="idle"] { color: #656d76; }
QLabel#ghSettingsStatus[state="validating"] { color: #0969da; }
QLabel#ghSettingsStatus[state="success"] { color: #1f883d; }
QLabel#ghSettingsStatus[state="error"] { color: #cf222e; }
QLabel#ghSourceLabel { color: #57606a; }
"""


def resource_path(relative_path: str) -> Path:
    base_path = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parents[1]))
    return base_path / relative_path


def configure_application(
    app: QApplication, icon_path: str | Path | None = None
) -> QIcon:
    app.setOrganizationName("VibeDev")
    app.setApplicationName("GithubDeck")
    app.setApplicationDisplayName("GithubDeck")
    app.setApplicationVersion("0.2.0")
    app.setStyleSheet(GITHUB_CONSOLE_STYLE_SHEET)
    icon = QIcon(str(icon_path or resource_path("assets/githubdeck-icon.png")))
    app.setWindowIcon(icon)
    return icon


def run() -> int:
    app = QApplication.instance() or QApplication(sys.argv)
    icon = configure_application(app)
    settings = GhExecutableSettings()
    process = GhProcess(settings)
    controller = AuthController(process)
    window = MainWindow(controller, settings, app_icon=icon)
    window.show()
    window.controller.refresh()
    return app.exec()
