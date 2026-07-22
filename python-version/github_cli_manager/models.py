from dataclasses import dataclass
from enum import Enum


class AuthState(str, Enum):
    AUTHENTICATED = "success"
    NEEDS_LOGIN = "failure"
    UNKNOWN = "unknown"


@dataclass(frozen=True, slots=True)
class Account:
    host: str
    login: str
    active: bool = False
    state: AuthState = AuthState.UNKNOWN
