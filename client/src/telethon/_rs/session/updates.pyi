from collections.abc import Buffer
from datetime import timedelta
from enum import Enum
from typing import final, Self, Sequence

from telethon._impl.tl import types

from .types import UpdatesState

@final
class GapError(Exception):
    pass

@final
class Instant:
    __slots__ = ()
    @staticmethod
    def now() -> Instant: ...
    def duration_since(self, earlier: Instant) -> timedelta: ...
    def checked_duration_since(self, earlier: Instant) -> timedelta | None: ...
    def saturating_duration_since(self, earlier: Instant) -> timedelta: ...
    def elapsed(self) -> timedelta: ...
    def __add__(self, other: timedelta, /) -> Instant: ...
    def __sub__(self, other: timedelta, /) -> Instant: ...

class UpdatesLike:
    @final
    class Updates(UpdatesLike):
        __slots__ = ()
        def __new__(cls, updates: Buffer) -> Self: ...
        @property
        def updates(self) -> bytes: ...  # abcs.Updates

    @final
    class ShortSentMessage(UpdatesLike):
        __slots__ = ()
        def __new__(cls, request: Buffer, update: Buffer) -> Self: ...
        @property
        def request(self) -> bytes: ...  # functions.messages.SendMessage
        @property
        def update(self) -> bytes: ...  # types.UpdateShortSentMessage

    @final
    class AffectedMessages(UpdatesLike):
        __slots__ = ()
        def __new__(cls, update: Buffer) -> Self: ...
        @property
        def update(self) -> bytes: ...  # types.messages.AffectedMessages

    @final
    class AffectedChannelMessages(UpdatesLike):
        __slots__ = ()
        def __new__(
            cls, affected: Buffer, channel_id: int, message_ids: Sequence[int]
        ) -> Self: ...
        @property
        def affected(self) -> bytes: ...  # tyoes.messages.AffectedMessages
        @property
        def channel_id(self) -> int: ...
        @property
        def message_ids(self) -> list[int]: ...

    @final
    class InvitedUsers(UpdatesLike):
        __slots__ = ()
        def __new__(cls, update: Buffer) -> Self: ...
        @property
        def update(self) -> bytes: ...  # types.messages.InvitedUsers

    @final
    class ChatInviteJoinResult(UpdatesLike):
        __slots__ = ()
        def __new__(cls, update: Buffer) -> Self: ...
        @property
        def update(self) -> bytes: ...  # types.messages.ChatInviteJoinResultOk

    @final
    class ConnectionClosed(UpdatesLike):
        __slots__ = ()
        def __new__(cls) -> Self: ...

    @final
    class MalformedUpdates(UpdatesLike):
        __slots__ = ()
        def __new__(cls) -> Self: ...

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

@final
class PrematureEndReason(Enum):
    TemporaryServerIssues = "TemporaryServerIssues"
    Banned = "Banned"

@final
class MessageBoxes:
    __slots__ = ()

    # ========================================
    # Creation, querying, and setting base state.
    # ========================================

    def __new__(cls, state: UpdatesState | None) -> Self: ...
    def session_state(self) -> UpdatesState: ...
    def is_empty(self) -> bool: ...
    def check_deadlines(self) -> Instant: ...
    def set_state(self, state: types.updates.State) -> None: ...
    def try_set_channel_state(self, id: int, pts: int) -> None: ...

    # ========================================
    # "Normal" updates flow (processing and detection of gaps).
    # ========================================

    def process_updates(self, updates: UpdatesLike) -> UpdateAndPeers: ...

    # ========================================
    # Getting and applying account difference.
    # ========================================

    def get_difference(self) -> bytes: ...  # functions.updates.GetDifference
    def apply_difference(
        self, difference: Buffer
    ) -> UpdateAndPeers: ...  # difference: abcs.updates.Difference

    # ========================================
    # Getting and applying channel difference.
    # ========================================

    def get_channel_difference(
        self,
    ) -> bytes: ...  # functions.updates.GetChannelDifference
    def apply_channel_difference(
        self, difference: Buffer
    ) -> UpdateAndPeers: ...  # difference: abcs.updates.ChannelDifference
    def end_channel_difference(self, reason: PrematureEndReason) -> None: ...
