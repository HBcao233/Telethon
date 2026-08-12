from abc import ABC, abstractmethod
from typing import Any, Self

from telethon._impl.session.types import (
    DcOption,
    PeerIdLike,
    PeerInfo,
    UpdatesState,
    UpdateState,
)

class Session(ABC):
    def __new__(cls, *args: Any, **kwargs: Any) -> Self: ...
    @abstractmethod
    def home_dc_id(self) -> int: ...
    @abstractmethod
    async def set_home_dc_id(self, dc_id: int) -> None: ...
    @abstractmethod
    def dc_option(self, dc_id: int) -> DcOption | None: ...
    @abstractmethod
    async def set_dc_option(self, dc_option: DcOption) -> None: ...
    @abstractmethod
    async def peer(self, peer: PeerIdLike) -> PeerInfo | None: ...
    @abstractmethod
    async def cache_peer(
        self, peer_info: PeerInfo.User | PeerInfo.Chat | PeerInfo.Channel
    ) -> None: ...
    @abstractmethod
    async def updates_state(self) -> UpdatesState: ...
    @abstractmethod
    async def set_update_state(
        self,
        update: UpdateState.All
        | UpdateState.Primary
        | UpdateState.Secondary
        | UpdateState.Channel,
    ) -> None: ...
