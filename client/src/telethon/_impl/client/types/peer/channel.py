from typing import Self

from telethon._impl.session import ChannelKind, PeerAuth, PeerId, PeerInfo, PeerRef
from telethon._impl.tl import abcs, types

from ..meta import NoPublicConstructor
from .peer import Peer


class Channel(Peer, metaclass=NoPublicConstructor):
    """
    A broadcast channel.

    You can get a channel from messages via :attr:`telethon.types.Message.chat`,
    or from methods such as :meth:`telethon.Client.resolve_username`.
    """

    def __init__(
        self,
        raw: types.Channel | types.ChannelForbidden,
    ) -> None:
        self._raw = raw

    @classmethod
    def _from_raw(cls, chat: abcs.Chat) -> Self:
        if isinstance(chat, (types.ChatEmpty, types.Chat, types.ChatForbidden)):
            raise TypeError("cannot create channel from group chat")
        elif isinstance(chat, (types.Channel, types.ChannelForbidden)):
            if not chat.broadcast:
                raise TypeError("cannot create group from broadcast channel")
            return cls._create(chat)
        else:
            raise TypeError("unexpected case")

    # region Overrides

    @property
    def id(self) -> PeerId:
        return PeerId.channel(self._raw.id)

    @property
    def name(self) -> str:
        """
        The channel's title.

        This property is always present, but may be the empty string.
        """
        return self._raw.title

    @property
    def username(self) -> str | None:
        return getattr(self._raw, "username", None)

    @property
    def access_hash(self) -> PeerAuth:
        return PeerAuth(getattr(self._raw, "access_hash", None) or 0)

    @property
    def ref(self) -> PeerRef:
        return PeerRef(self.id, self.access_hash)

    @property
    def _ref(self) -> PeerRef:
        return self.ref

    @property
    def _info(self) -> PeerInfo.Channel:
        return PeerInfo.Channel(
            id=self.id.bare_id,
            auth=self.access_hash,
            kind=self.kind,
        )

    # endregion Overrides

    @property
    def kind(self) -> ChannelKind:
        return ChannelKind.Broadcast
