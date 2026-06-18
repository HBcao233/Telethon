from telethon._rs import (
    PeerId,
    PeerAuth,
    PeerInfo,
    PeerKind,
    ChannelKind,
    DcOption,
    ChannelState,
    UpdatesState,
    UpdateState,
)
from typing import TypeAlias
from typing_extensions import assert_never

from telethon import _rs
from telethon._impl.tl import abcs, types

PeerIdLike: TypeAlias = int | PeerId
PeerAuthLike: TypeAlias = int | PeerAuth

__all__ = [
    "PeerId",
    "PeerIdLike",
    "PeerAuth",
    "PeerAuthLike",
    "PeerInfo",
    "PeerKind",
    "PeerRef",
    "ChannelKind",
    "DcOption",
    "ChannelState",
    "UpdateState",
    "UpdatesState",
]


class PeerRef(_rs.session.PeerRef):
    @property
    def _ref(self) -> "PeerRef":
        return self

    @classmethod
    def _empty_from_peer(cls, peer: abcs.Peer) -> "PeerRef":
        if isinstance(peer, types.PeerUser):
            return PeerRef(PeerId.user(peer.user_id), 0)
        elif isinstance(peer, types.PeerChat):
            return PeerRef(PeerId.chat(peer.chat_id), 0)
        elif isinstance(peer, types.PeerChannel):
            return PeerRef(PeerId.channel(peer.channel_id), 0)
        else:
            raise RuntimeError("unexpected case")

    def _to_peer(self) -> abcs.Peer:
        match self.id.kind:
            case PeerKind.UserSelf:
                raise ValueError(f"can't cast '{self}' to Peer")
            case PeerKind.User:
                return types.PeerUser(
                    user_id=self.id.bare_id,
                )
            case PeerKind.Chat:
                return types.PeerChat(
                    chat_id=self.id.bare_id,
                )
            case PeerKind.Channel:
                return types.PeerChannel(
                    channel_id=self.id.bare_id,
                )
            case _ as unreachable:
                assert_never(unreachable)

    def _to_input_peer(self) -> abcs.InputPeer:
        match self.id.kind:
            case PeerKind.UserSelf:
                return types.InputPeerSelf()
            case PeerKind.User:
                return types.InputPeerUser(
                    user_id=self.id.bare_id,
                    access_hash=int(self.access_hash),
                )
            case PeerKind.Chat:
                return types.InputPeerChat(
                    chat_id=self.id.bare_id,
                )
            case PeerKind.Channel:
                return types.InputPeerChannel(
                    channel_id=self.id.bare_id,
                    access_hash=int(self.access_hash),
                )
            case _ as unreachable:
                assert_never(unreachable)

    def _to_input_user(self) -> abcs.InputUser:
        match self.id.kind:
            case PeerKind.UserSelf:
                return types.InputUserSelf()
            case PeerKind.User:
                return types.InputUser(
                    user_id=self.id.bare_id,
                    access_hash=int(self.access_hash),
                )
            case _:
                raise ValueError("can't cast '{self}' to InputUser")

    def _to_input_chat(self) -> int:
        match self.id.kind:
            case PeerKind.Chat:
                return self.id.bare_id
            case _:
                raise ValueError("'{self}' is not a chat")

    def _to_input_channel(self) -> abcs.InputChannel:
        match self.id.kind:
            case PeerKind.Channel:
                return types.InputChannel(
                    channel_id=self.id.bare_id,
                    access_hash=int(self.access_hash),
                )
            case _:
                raise ValueError("can't cast '{self}' to InputChannel")
