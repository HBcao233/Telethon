from __future__ import annotations

import asyncio
import logging
import platform
import re
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, TypeVar

from telethon._impl.mtsender import RpcError, SenderPool
from telethon._impl.tl import Request, functions, types
from telethon.version import __version__

from ..errors import adapt_rpc
from .updates import dispatcher, process_socket_updates

if TYPE_CHECKING:
    from .client import Client


Return = TypeVar("Return")


def default_device_model() -> str:
    system = platform.uname()
    if system.machine in ("x86_64", "AMD64"):
        return "PC 64bit"
    elif system.machine in ("i386", "i686", "x86"):
        return "PC 32bit"
    else:
        return system.machine or "Unknown"


def default_system_version() -> str:
    system = platform.uname()
    return re.sub(r"-.+", "", system.release) or "1.0"


@dataclass
class Config:
    api_id: int
    api_hash: str
    base_logger: logging.Logger
    # TODO
    # reconnection_policy: Optional[ReconnectionPolicy] = None
    use_ipv6: bool
    device_model: str = field(default_factory=default_device_model)
    system_version: str = field(default_factory=default_system_version)
    app_version: str = __version__
    system_lang_code: str = "en"
    lang_code: str = "en"
    catch_up: bool = False
    # TODO
    # datacenter: Optional[DataCenter] = None
    flood_sleep_threshold: int = 60
    update_queue_limit: int | None = None


async def connect(self: Client) -> None:
    if self._sender is not None:
        return

    self._sender = SenderPool(
        session=self._session,
        api_id=self._config.api_id,
        use_ipv6=self._config.use_ipv6,
        device_model=self._config.device_model,
        system_version=self._config.system_version,
        app_version=self._config.app_version,
        system_lang_code=self._config.system_lang_code,
        lang_code=self._config.lang_code,
        catch_up=self._config.catch_up,
    )
    self._dispatcher = asyncio.create_task(dispatcher(self))


async def disconnect(self: Client) -> None:
    assert self._sender is not None
    assert self._dispatcher

    sender = self._sender
    self._sender = None

    self._dispatcher.cancel()
    try:
        await self._dispatcher
    except asyncio.CancelledError:
        pass
    except Exception:
        self._config.base_logger.exception(
            "unhandled exception when cancelling dispatcher; this is a bug"
        )
    finally:
        self._dispatcher = None

    try:
        await sender.disconnect()
    except Exception:
        self._config.base_logger.exception(
            "unhandled exception during disconnect; this is a bug"
        )


async def invoke_in_dc[Request](
    client: Client,
    dc_id: int,
    request: Request[Return],
) -> Return:
    assert client._sender is not None

    sleep_thresh = client._config.flood_sleep_threshold

    sender = client._sender
    try:
        response = await sender.invoke_in_dc(dc_id, request)
    except RpcError as e:
        if e.code == 420 and e.value is not None and e.value < sleep_thresh:
            await asyncio.sleep(e.value)
            sleep_thresh -= e.value
            response = await sender.invoke_in_dc(dc_id, request)
        else:
            raise adapt_rpc(e) from None

    return request.deserialize_response(response)


async def invoke[Request](
    client: Client,
    request: Request[Return],
) -> Return:
    return await invoke_in_dc(client, client._session.home_dc_id(), request)


async def step_sender(client: Client) -> None:
    assert client._sender is not None

    try:
        updates = await client._sender.pop_updates()
    except ConnectionError:
        if client.connected:
            raise
        else:
            # disconnect was called, so the socket returning 0 bytes is expected
            return

    process_socket_updates(client, updates)


async def run_until_disconnected(self: Client) -> None:
    while self.connected:
        if self._sender:
            await step_sender(self)


async def sync_update_state(self: Client) -> None:
    assert self._sender is not None

    await self._sender.sync_update_state()


def connected(self: Client) -> bool:
    return self._sender is not None


async def copy_auth_to_dc(self: Client, target_dc_id: int) -> bool:
    if target_dc_id in self._auth_copied_to_dcs:
        return False

    exported_auth = await self.invoke(
        functions.auth.export_authorization(dc_id=target_dc_id)
    )
    assert isinstance(exported_auth, types.auth.ExportedAuthorization)
    await self.invoke_in_dc(
        target_dc_id,
        functions.auth.import_authorization(
            id=exported_auth.id,
            bytes=exported_auth.bytes,
        ),
    )

    self._auth_copied_to_dcs.append(target_dc_id)

    return True
