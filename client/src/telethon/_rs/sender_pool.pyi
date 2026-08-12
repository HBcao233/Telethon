from collections.abc import Buffer
from typing import Self, final

from .session import Session
from .session.updates import MessageBoxes, UpdateAndPeers

@final
class SenderPool:
    def __new__(
        cls,
        session: Session,
        api_id: int,
        *,
        use_ipv6: bool = False,
        device_model: str = "",
        system_version: str = "",
        app_version: str = "",
        system_lang_code: str = "",
        lang_code: str = "",
    ) -> Self: ...
    @property
    def session(self) -> Session: ...
    @property
    def home_dc_id(self) -> int: ...
    @property
    def api_id(self) -> int: ...
    @property
    def use_ipv6(self) -> bool: ...
    @property
    def device_model(self) -> str: ...
    @property
    def system_version(self) -> str: ...
    @property
    def app_version(self) -> str: ...
    @property
    def system_lang_code(self) -> str: ...
    @property
    def lang_code(self) -> str: ...
    @property
    def _message_box(self) -> MessageBoxes: ...
    async def is_connected(self) -> None: ...
    async def invoke_in_dc(self, dc_id: int, body: Buffer) -> bytes: ...
    async def invoke(self, body: Buffer) -> bytes: ...
    async def disconnect(self) -> None: ...
    async def pop_updates(self) -> UpdateAndPeers:
        """
        Pops updates from the queue, waiting for updates to arrive.
        """
    async def sync_update_state(self) -> None:
        """
        Synchronize the updates state to the session.

        This is **not** automatically done on disconnect.
        """
