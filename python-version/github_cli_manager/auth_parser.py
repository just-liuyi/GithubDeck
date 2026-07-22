import json
from typing import Any

from .models import Account, AuthState


class AuthStatusError(ValueError):
    """Raised when gh auth status returns an unexpected payload."""


_STATE_WHITELIST = {
    "success": AuthState.AUTHENTICATED,
    "failure": AuthState.NEEDS_LOGIN,
}


def parse_auth_status(payload: bytes | str) -> list[Account]:
    try:
        data: Any = json.loads(payload)
    except (json.JSONDecodeError, UnicodeDecodeError, TypeError) as exc:
        raise AuthStatusError("GitHub CLI 返回了无法解析的状态") from exc

    hosts = data.get("hosts") if isinstance(data, dict) else None
    if not isinstance(hosts, dict):
        raise AuthStatusError("GitHub CLI 状态中缺少 hosts")

    accounts: list[Account] = []
    for host, entries in hosts.items():
        if not isinstance(host, str) or not isinstance(entries, list):
            continue
        for entry in entries:
            if not isinstance(entry, dict):
                continue
            login = entry.get("login")
            if isinstance(login, str) and login:
                accounts.append(
                    Account(
                        host=host,
                        login=login,
                        active=entry.get("active") is True,
                        state=_STATE_WHITELIST.get(entry.get("state"), AuthState.UNKNOWN),
                    )
                )

    return sorted(accounts, key=lambda account: (account.host, not account.active, account.login.casefold()))
