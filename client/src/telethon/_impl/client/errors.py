import re

from telethon._impl.mtsender import RpcError


def canonicalize_code(code: int) -> int:
    return abs(code)  # -503 Timeout -> 503


def canonicalize_name(name: str) -> str:
    return name.upper()  # -503 Timeout -> TIMEOUT


def adapt_user_name(name: str) -> str:
    return re.sub(r"[A-Z]", lambda m: "_" + m[0].lower(), name).strip("_").upper()


def pretty_name(name: str) -> str:
    return "".join(map(str.title, name.split("_")))


def from_code(
    code: int,
    *,
    _cache: dict[int, type[RpcError]] | None = None,
) -> type[RpcError]:
    if _cache is None:
        _cache = {}

    code = canonicalize_code(code)
    if code not in _cache:
        _cache[code] = type(f"Code{code}", (RpcError,), {})
    return _cache[code]


def from_name(
    name: str,
    *,
    _cache: dict[str, type[RpcError]] | None = None,
) -> type[RpcError]:
    if _cache is None:
        _cache = {}

    name = canonicalize_name(name)
    if name not in _cache:
        _cache[name] = type(pretty_name(name), (RpcError,), {})
    return _cache[name]


def adapt_rpc(
    error: RpcError,
    *,
    _cache: dict[tuple[int, str], type[RpcError]] | None = None,
) -> RpcError:
    if _cache is None:
        _cache = {}

    code = canonicalize_code(error.code)
    name = canonicalize_name(error.name)
    tup = code, name
    if tup not in _cache:
        _cache[tup] = type(pretty_name(name), (from_code(code), from_name(name)), {})
    return _cache[tup](
        code=error.code, name=error.name, value=error.value, caused_by=error.caused_by
    )


class ErrorFactory:
    __slots__ = ()

    def __getattr__(self, name: str) -> type[RpcError]:
        if m := re.match(r"Code(\d+)$", name):
            return from_code(int(m[1]))
        else:
            adapted = adapt_user_name(name)
            if pretty_name(canonicalize_name(adapted)) != name or re.match(
                r"[A-Z]{2}", name
            ):
                raise AttributeError(f"error subclass names must be CamelCase: {name}")
            return from_name(adapted)


errors = ErrorFactory()
