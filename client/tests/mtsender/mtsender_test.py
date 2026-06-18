import asyncio
import logging

from pytest import LogCaptureFixture, mark

from telethon._impl.session import SqliteSession
from telethon._impl.mtsender import SenderPool
from telethon._impl.tl import LAYER, functions

API_ID = 4
TELEGRAM_TEST_DC = 2, "149.154.167.40:443"

TEST_TIMEOUT = 10000


@mark.net
async def test_invoke_encrypted_method(caplog: LogCaptureFixture) -> None:
    caplog.set_level(logging.DEBUG)

    session = SqliteSession(":memory:")
    sender_pool = SenderPool(session, api_id=API_ID)

    request = functions.invoke_with_layer(
        layer=LAYER,
        query=functions.init_connection(
            api_id=1,
            device_model="Test",
            system_version="0.1",
            app_version="0.1",
            system_lang_code="en",
            lang_pack="",
            lang_code="",
            proxy=None,
            params=None,
            query=functions.help.get_nearest_dc(),
        ),
    )

    response = await asyncio.wait_for(
        sender_pool.invoke(request),
        TEST_TIMEOUT,
    )
    res = request.deserialize_response(response)
    assert res.country
    assert res.this_dc
    assert res.nearest_dc
