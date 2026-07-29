from __future__ import annotations

import asyncio
import itertools
from typing import TYPE_CHECKING, Optional, Iterable, TypeAlias

from telethon._impl.session import PeerId
from telethon._impl.tl import abcs, types
from .channel import Channel
from .group import Group
from .peer import Peer
from .user import User

if TYPE_CHECKING:
    from ...client.client import Client

PeerMap: TypeAlias = dict[PeerId, Peer]


def build_chat_map(
    client: Client,
    users: Iterable[abcs.User],
    chats: Iterable[abcs.Chat],
) -> dict[PeerId, Peer]:
    users_iter = (User._from_raw(u) for u in users)
    chats_iter = (
        (
            Channel._from_raw(c)
            if isinstance(c, (types.Channel, types.ChannelForbidden)) and c.broadcast
            else Group._from_raw(client, c)
        )
        for c in chats
    )

    result: dict[PeerId, Peer] = {
        c.id: c for c in itertools.chain(users_iter, chats_iter)
    }

    async def _safe_cache_peer(peer: Peer) -> None:
        try:
            await client._session.cache_peer(peer._info)
        except Exception:
            client._config.base_logger.warning(
                f"Failed to cache peer {peer.id}",
                exc_info=True
            )

    for peer in result.values():
        asyncio.create_task(_safe_cache_peer(peer))

    return result


def peer_id(peer: abcs.Peer) -> PeerId:
    if isinstance(peer, types.PeerUser):
        return PeerId.user(peer.user_id)
    elif isinstance(peer, types.PeerChat):
        return PeerId.chat(peer.chat_id)
    elif isinstance(peer, types.PeerChannel):
        return PeerId.channel(peer.channel_id)
    else:
        raise RuntimeError("unexpected case")


def expand_peer(client: Client, peer: abcs.Peer, *, broadcast: Optional[bool]) -> Peer:
    if isinstance(peer, types.PeerUser):
        return User._from_raw(types.UserEmpty(id=peer.user_id))
    elif isinstance(peer, types.PeerChat):
        return Group._from_raw(client, types.ChatEmpty(id=peer.chat_id))
    elif isinstance(peer, types.PeerChannel):
        if broadcast is None:
            broadcast = True  # assume broadcast by default (Channel type is more accurate than Group)

        channel = types.ChannelForbidden(
            broadcast=broadcast,
            megagroup=not broadcast,
            id=peer.channel_id,
            access_hash=0,
            title="",
            until_date=None,
        )

        return (
            Channel._from_raw(channel)
            if broadcast
            else Group._from_raw(client, channel)
        )
    else:
        raise RuntimeError("unexpected case")


__all__ = ["Channel", "Peer", "Group", "User"]
