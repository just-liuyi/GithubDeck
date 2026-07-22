from github_cli_manager.gh_process import (
    _LOGOUT_TIMEOUT_MS,
    _STATUS_TIMEOUT_MS,
    _SWITCH_TIMEOUT_MS,
    GhProcess,
)


class FakeSettings:
    def __init__(self, path: str) -> None:
        self.value = path

    def path(self) -> str:
        return self.value


def make_process(monkeypatch, tmp_path) -> tuple[GhProcess, FakeSettings]:
    executable = tmp_path / "gh.exe"
    executable.write_bytes(b"")
    settings = FakeSettings(str(executable))
    process = GhProcess(settings)
    monkeypatch.setattr(process._process, "start", lambda: None)
    return process, settings


def test_status_and_switch_start_operation_timeout(monkeypatch, tmp_path, qapp) -> None:
    process, _settings = make_process(monkeypatch, tmp_path)

    assert process.refresh()
    assert process._operation_timer.interval() == _STATUS_TIMEOUT_MS
    assert process._operation_timer.isActive()

    process._operation_timer.stop()
    assert process.switch("github.com", "alice")
    assert process._operation_timer.interval() == _SWITCH_TIMEOUT_MS
    assert process._operation_timer.isActive()


def test_logout_uses_exact_account_and_timeout(monkeypatch, tmp_path, qapp) -> None:
    process, _settings = make_process(monkeypatch, tmp_path)

    assert process.logout("github.com", "alice")

    assert process._process.arguments() == [
        "auth",
        "logout",
        "--hostname",
        "github.com",
        "--user",
        "alice",
    ]
    assert process._operation_timer.interval() == _LOGOUT_TIMEOUT_MS
    assert process._operation_timer.isActive()


def test_all_operations_read_current_executable_path(monkeypatch, tmp_path) -> None:
    process, settings = make_process(monkeypatch, tmp_path)
    first = settings.value
    second = tmp_path / "other" / "gh.exe"
    second.parent.mkdir()
    second.write_bytes(b"")

    assert process.refresh()
    assert process._process.program() == first
    process._operation_timer.stop()

    settings.value = str(second)
    assert process.switch("github.com", "alice")
    assert process._process.program() == str(second)
    process._operation_timer.stop()

    assert process.login()
    assert process._process.program() == str(second)


def test_login_uses_system_browser_and_has_no_short_timeout(monkeypatch, tmp_path) -> None:
    process, _settings = make_process(monkeypatch, tmp_path)
    monkeypatch.setenv("GH_BROWSER", "inherited.exe")

    assert process.login()
    assert process._process.arguments() == [
        "auth",
        "login",
        "--hostname",
        "github.com",
        "--git-protocol",
        "https",
        "--web",
    ]
    assert not process._process.processEnvironment().contains("GH_BROWSER")
    assert not process._operation_timer.isActive()


def test_each_operation_rebuilds_sanitized_environment(monkeypatch, tmp_path) -> None:
    process, _settings = make_process(monkeypatch, tmp_path)
    monkeypatch.setenv("GH_TOKEN", "secret")
    monkeypatch.setenv("GITHUB_TOKEN", "secret")
    monkeypatch.setenv("GH_ENTERPRISE_TOKEN", "secret")
    monkeypatch.setenv("GITHUB_ENTERPRISE_TOKEN", "secret")
    monkeypatch.setenv("GH_BROWSER", "inherited.exe")

    assert process.login()
    assert process.refresh()
    environment = process._process.processEnvironment()

    assert not environment.contains("GH_BROWSER")
    assert all(
        not environment.contains(name)
        for name in (
            "GH_TOKEN",
            "GITHUB_TOKEN",
            "GH_ENTERPRISE_TOKEN",
            "GITHUB_ENTERPRISE_TOKEN",
        )
    )


def test_missing_configured_executable_is_reported_unavailable(tmp_path) -> None:
    process = GhProcess(FakeSettings(str(tmp_path / "missing" / "gh.exe")))

    assert not process.executable_available


def test_cancelled_login_ignores_late_device_code() -> None:
    process = GhProcess(FakeSettings("missing-gh.exe"))
    codes: list[str] = []
    process.authorization_code.connect(codes.append)
    process._operation = "login"
    process._cancelled = True

    process._consume_login_output(b"late ABCD-EF12 after cancel")

    assert codes == []
    assert process._login_buffer == b""


def test_device_code_parser_handles_streams_ansi_and_rejects_bad_formats(monkeypatch) -> None:
    process = GhProcess(FakeSettings("missing-gh.exe"))
    codes: list[str] = []
    writes: list[bytes] = []
    process.authorization_code.connect(codes.append)
    monkeypatch.setattr(process._process, "write", writes.append)
    process._operation = "login"

    process._consume_login_output(b"visit https://example.invalid and enter AB")
    process._consume_login_output(b"\x1b[32mCD-EF12\x1b[0m now")
    process._consume_login_output(b" duplicate ABCD-EF12")

    assert codes == ["ABCD-EF12"]
    assert writes == [b"\n"]

    process._clear_login_state()
    for invalid in (b"abcd-EF12", b"ABC-EF12", b"ABCDE-EF12", b"ABCD-EF1!", b"XABCD-EF12Y"):
        process._consume_login_output(invalid)
    assert codes == ["ABCD-EF12"]
