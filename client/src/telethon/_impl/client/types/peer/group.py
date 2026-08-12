from __future__ import annotations

import datetime
from collections.abc import Sequence
from typing import TYPE_CHECKING, Self

from telethon._impl.session import ChannelKind, PeerAuth, PeerId, PeerInfo, PeerRef

from ....tl import abcs, types
from ..chat_restriction import ChatRestriction
from ..meta import NoPublicConstructor
from .peer import Peer

if TYPE_CHECKING:
    from ...client.client import Client


class Group(Peer, metaclass=NoPublicConstructor):
    """
    A small group or supergroup.

    You can get a group from messages via :attr:`telethon.types.Message.chat`,
    or from methods such as :meth:`telethon.Client.resolve_username`.
    """

    def __init__(
        self,
        client: Client,
        chat: (
            types.ChatEmpty
            | types.Chat
            | types.ChatForbidden
            | types.Channel
            | types.ChannelForbidden
        ),
    ) -> None:
        self._client = client
        self._raw = chat

    @classmethod
    def _from_raw(cls, client: Client, chat: abcs.Chat) -> Self:
        if isinstance(chat, (types.ChatEmpty, types.Chat, types.ChatForbidden)):
            return cls._create(client, chat)
        elif isinstance(chat, (types.Channel, types.ChannelForbidden)):
            if chat.broadcast:
                raise TypeError("cannot create group from broadcast channel")
            return cls._create(client, chat)
        else:
            raise TypeError("unexpected case")

    # region Overrides

    @property
    def id(self) -> PeerId:
        if self.is_megagroup:
            return PeerId.channel(self._raw.id)
        else:
            return PeerId.chat(self._raw.id)

    @property
    def name(self) -> str:
        """
        The group's title.

        This property is always present, but may be the empty string.
        """
        return getattr(self._raw, "title", None) or ""

    @property
    def username(self) -> str | None:
        return getattr(self._raw, "username", None)

    @property
    def access_hash(self) -> PeerAuth:
        return PeerAuth(getattr(self._raw, "access_hash", None) or 0)

    @property
    def ref(self) -> PeerRef:
        # if isinstance(self._raw, (types.ChatEmpty, types.Chat, types.ChatForbidden)):
        return PeerRef(self.id, self.access_hash)

    @property
    def _ref(self) -> PeerRef:
        return self.ref

    @property
    def _info(self) -> PeerInfo.Chat | PeerInfo.Channel:
        try:
            channel_kind = self.kind
        except TypeError:
            return PeerInfo.Chat(id=self.id.bare_id)
        else:
            return PeerInfo.Channel(
                id=self.id.bare_id,
                auth=self.access_hash,
                kind=channel_kind,
            )

    # endregion Overrides

    @property
    def is_megagroup(self) -> bool:
        """
        Whether the group is a supergroup.

        These are known as "megagroups" in Telegram's API, and are different from "gigagroups".
        """
        return isinstance(self._raw, (types.Channel, types.ChannelForbidden))

    @property
    def kind(self) -> ChannelKind:
        if getattr(self._raw, "gigagroup", None):
            return ChannelKind.Gigagroup
        elif getattr(self._raw, "megagroup", None):
            return ChannelKind.Broadcast
        else:
            raise TypeError("This group not a channel.")

    async def set_default_restrictions(
        self,
        restrictions: Sequence[ChatRestriction],
        *,
        until: datetime.datetime | None = None,
    ) -> None:
        """
        Alias for :meth:`telethon.Client.set_chat_default_restrictions`.
        """
        await self._client.set_chat_default_restrictions(
            self, restrictions, until=until
        )
