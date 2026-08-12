from __future__ import annotations

from collections.abc import AsyncIterator
from typing import TYPE_CHECKING, Self

from telethon._impl.session import PeerRef
from telethon._impl.tl import abcs, functions, types

from ..types import InlineResult, NoPublicConstructor, Peer, User

if TYPE_CHECKING:
    from .client import Client


class InlineResults(metaclass=NoPublicConstructor):
    def __init__(
        self,
        client: Client,
        bot: abcs.InputUser,
        query: str,
        peer: PeerRef | None,
    ) -> None:
        self._client = client
        self._bot = bot
        self._query = query
        self._peer = peer
        self._offset: str | None = ""
        self._buffer: list[InlineResult] = []
        self._done = False

    def __aiter__(self) -> Self:
        return self

    async def __anext__(self) -> InlineResult:
        if not self._buffer:
            if self._offset is None:
                raise StopAsyncIteration

            result = await self._client(
                functions.messages.get_inline_bot_results(
                    bot=self._bot,
                    peer=(
                        self._peer._to_input_peer()
                        if self._peer
                        else types.InputPeerEmpty()
                    ),
                    geo_point=None,
                    query=self._query,
                    offset=self._offset,
                )
            )
            assert isinstance(result, types.messages.BotResults)
            self._offset = result.next_offset
            for r in reversed(result.results):
                assert isinstance(
                    r, (types.BotInlineMediaResult, types.BotInlineResult)
                )
                self._buffer.append(
                    InlineResult._create(self._client, result, r, self._peer)
                )

        if not self._buffer:
            self._offset = None
            raise StopAsyncIteration

        return self._buffer.pop()


async def inline_query(
    self: Client,
    bot: User | PeerRef,
    /,
    query: str = "",
    *,
    peer: Peer | PeerRef | None = None,
) -> AsyncIterator[InlineResult]:
    input_user = bot._ref._to_input_user()

    return InlineResults._create(
        self,
        input_user,
        query,
        peer._ref if peer else None,
    )
