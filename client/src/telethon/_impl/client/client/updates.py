from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable
from inspect import isawaitable
from typing import TYPE_CHECKING, Any, Optional, Sequence, Type, Iterable

from telethon._impl.session import GapError, State, UpdatesLike
from telethon._impl.tl import abcs
from telethon._impl.tl.core import Reader
from ..events import Continue, Event
from ..events.filters import FilterType
from ..types import build_chat_map

if TYPE_CHECKING:
    from .client import Client


UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN = 300


def on(
    self: Client, event_cls: Type[Event], /, filter: Optional[FilterType] = None
) -> Callable[[Callable[[Event], Awaitable[Any]]], Callable[[Event], Awaitable[Any]]]:
    def wrapper(
        handler: Callable[[Event], Awaitable[Any]],
    ) -> Callable[[Event], Awaitable[Any]]:
        add_event_handler(self, handler, event_cls, filter)
        return handler

    return wrapper


def add_event_handler(
    self: Client,
    handler: Callable[[Event], Awaitable[Any]],
    /,
    event_cls: Type[Event],
    filter: Optional[FilterType] = None,
) -> None:
    self._handlers.setdefault(event_cls, []).append((handler, filter))


def remove_event_handler(
    self: Client, handler: Callable[[Event], Awaitable[Any]], /
) -> None:
    for event_cls, handlers in tuple(self._handlers.items()):
        for i in reversed(range(len(handlers))):
            if handlers[i][0] == handler:
                handlers.pop(i)
        if not handlers:
            del self._handlers[event_cls]


def get_handler_filter(
    self: Client, handler: Callable[[Event], Awaitable[Any]], /
) -> Optional[FilterType]:
    for handlers in self._handlers.values():
        for h, f in handlers:
            if h == handler:
                return f
    return None


def set_handler_filter(
    self: Client,
    handler: Callable[[Event], Awaitable[Any]],
    /,
    filter: Optional[FilterType] = None,
) -> None:
    for handlers in self._handlers.values():
        for i, (h, _) in enumerate(handlers):
            if h == handler:
                handlers[i] = (h, filter)


def process_socket_updates(
    client: Client,
    x: UpdateAndPeers,
) -> None:
    def serialize_update(u: tuple[bytes, State]) -> tuple[abcs.Update, State]:
        update, state = u
        result = Reader(update).read_serializable(abcs.Update)  # type: ignore[type-abstract]
        return (result, state)

    def serialize_user(user: bytes) -> abcs.User:
        return Reader(user).read_serializable(abcs.User)  # type: ignore[type-abstract]

    def serialize_chat(chat: bytes) -> abcs.Chat:
        return Reader(chat).read_serializable(abcs.Chat)  # type: ignore[type-abstract]

    result = map(serialize_update, x.updates)
    users = map(serialize_user, x.users)
    chats = map(serialize_chat, x.chats)

    extend_update_queue(client, result, users, chats)


def extend_update_queue(
    client: Client,
    updates: Iterable[tuple[abcs.Update, State]],
    users: Iterable[abcs.User],
    chats: Iterable[abcs.Chat],
) -> None:
    chat_map = build_chat_map(client, users, chats)

    for update, state in updates:
        try:
            client._updates.put_nowait((update, state, chat_map))
        except asyncio.QueueFull:
            now = asyncio.get_running_loop().time()
            if client._last_update_limit_warn is None or (
                now - client._last_update_limit_warn
                > UPDATE_LIMIT_EXCEEDED_LOG_COOLDOWN
            ):
                client._config.base_logger.warning(
                    "updates are being dropped because limit=%d has been reached",
                    client._updates.maxsize,
                )
                client._last_update_limit_warn = now
            break


async def dispatcher(client: Client) -> None:
    loop = asyncio.get_running_loop()
    while client.connected:
        try:
            await dispatch_next(client)
        except asyncio.CancelledError:
            raise
        except Exception as e:
            if isinstance(e, RuntimeError) and loop.is_closed():
                # User probably forgot to call disconnect.
                client._config.base_logger.warning(
                    "client was not closed cleanly, make sure to call client.disconnect()! %s",
                    e,
                )
                return
            else:
                client._config.base_logger.exception(
                    "unhandled exception in event handler; this is probably a bug in your code, not telethon"
                )


async def dispatch_next(client: Client) -> None:
    # TODO: state
    update, state, chat_map = await client._updates.get()
    for event_cls, handlers in client._handlers.items():
        if event := event_cls._try_from_update(client, update, chat_map):
            for handler, filter in handlers:
                if not filter or (await r if isawaitable(r := filter(event)) else r):
                    ret = await handler(event)
                    if not (ret is Continue or client._check_all_handlers):
                        return
