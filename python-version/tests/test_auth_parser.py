import pytest

from github_cli_manager.auth_parser import AuthStatusError, parse_auth_status
from github_cli_manager.models import Account, AuthState


def test_parse_accounts_and_active_state() -> None:
    payload = b'''{"hosts":{"github.com":[
        {"login":"second","active":false},
        {"login":"first","active":true}
    ],"example.com":[{"login":"enterprise","active":true}]}}'''

    assert parse_auth_status(payload) == [
        Account("example.com", "enterprise", True),
        Account("github.com", "first", True),
        Account("github.com", "second", False),
    ]


def test_parse_empty_hosts() -> None:
    assert parse_auth_status('{"hosts": {}}') == []


def test_state_is_parsed_only_from_whitelist() -> None:
    payload = '''{"hosts":{"github.com":[
        {"login":"valid","state":"success"},
        {"login":"expired","state":"failure"},
        {"login":"other","state":"attacker-controlled"}
    ]}}'''

    accounts = parse_auth_status(payload)
    assert [account.state for account in accounts] == [
        AuthState.NEEDS_LOGIN,
        AuthState.UNKNOWN,
        AuthState.AUTHENTICATED,
    ]


@pytest.mark.parametrize("payload", ["not-json", "[]", '{"other": {}}'])
def test_reject_invalid_status(payload: str) -> None:
    with pytest.raises(AuthStatusError):
        parse_auth_status(payload)
