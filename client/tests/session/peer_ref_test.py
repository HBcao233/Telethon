from pytest import raises
from telethon._impl.session import PeerAuth, PeerId, PeerKind, PeerRef
from telethon._impl.tl import types

USER = PeerRef(PeerId.user(12), 34)
CHAT = PeerRef(PeerId.chat(5), 0)
CHANNEL = PeerRef(PeerId.channel(67), 89)
USER_SELF = PeerRef(PeerId.self_user(), 0)


def test_peer_ref() -> None:
    assert USER.kind == PeerKind.User
    assert CHAT.kind == PeerKind.Chat
    assert CHANNEL.kind == PeerKind.Channel
    assert USER_SELF.kind == PeerKind.UserSelf

    assert USER.access_hash == PeerAuth(34)
    assert CHAT.access_hash == PeerAuth(0)
    assert CHANNEL.access_hash == PeerAuth(89)

    assert USER._ref is USER

    assert USER._to_input_peer() == types.InputPeerUser(
        user_id=12,
        access_hash=34,
    )
    assert USER._to_input_user() == types.InputUser(
        user_id=12,
        access_hash=34,
    )
    with raises(ValueError):
        USER._to_input_chat()
    with raises(ValueError):
        USER._to_input_channel()

    assert CHAT._to_input_peer() == types.InputPeerChat(
        chat_id=5,
    )
    with raises(ValueError):
        CHAT._to_input_user()
    assert CHAT._to_input_chat() == 5
    with raises(ValueError):
        CHAT._to_input_channel()

    assert CHANNEL._to_input_peer() == types.InputPeerChannel(
        channel_id=67,
        access_hash=89,
    )
    with raises(ValueError):
        CHANNEL._to_input_user()
    with raises(ValueError):
        CHANNEL._to_input_chat()
    assert CHANNEL._to_input_channel() == types.InputChannel(
        channel_id=67,
        access_hash=89,
    )
