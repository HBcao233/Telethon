from collections.abc import Buffer, Sequence
from typing import Self, final

class MessageBox:
    @final
    class Common(MessageBox):
        __slots__ = ()
        def __new__(cls, pts: int) -> Self: ...
        @property
        def pts(self) -> int: ...

    @final
    class Secondary(MessageBox):
        __slots__ = ()
        def __new__(cls, qts: int) -> Self: ...
        @property
        def qts(self) -> int: ...

    @final
    class Channel(MessageBox):
        __slots__ = ()
        def __new__(cls, channel_id: int, pts: int) -> Self: ...
        @property
        def channel_id(self) -> int: ...
        @property
        def pts(self) -> int: ...

@final
class State:
    __slots__ = ()

    def __new__(cls, date: int, seq: int, message_box: MessageBox | None) -> Self: ...
    @property
    def date(self) -> int: ...
    @property
    def seq(self) -> int: ...
    @property
    def message_box(self) -> MessageBox | None: ...

@final
class UpdateAndPeers:
    __slots__ = ()

    def __new__(
        cls,
        updates: Sequence[tuple[Buffer, State]],
        users: Sequence[Buffer],
        chats: Sequence[Buffer],
    ) -> Self: ...
    @property
    def updates(self) -> list[tuple[bytes, State]]: ...  # (abcs.Update, State)
    @property
    def users(self) -> list[bytes]: ...  # abcs.User
    @property
    def chats(self) -> list[bytes]: ...  # abcs.Chat
