import struct
from collections.abc import Callable
from typing import Any, Generic, Optional, TypeVar

Return = TypeVar("Return")


def _bootstrap_get_deserializer(
    constructor_id: int,
) -> Optional[Callable[[bytes], Any]]:
    # Similar to Reader's bootstrapping.
    if Request._get_deserializer is _bootstrap_get_deserializer:
        from ..layer import RESPONSE_MAPPING as API_DESER
        from ..mtproto.layer import RESPONSE_MAPPING as MTPROTO_DESER

        if API_DESER.keys() & MTPROTO_DESER.keys():
            raise RuntimeError(
                "generated api and mtproto schemas cannot have colliding constructor identifiers"
            )
        all_deser = API_DESER | MTPROTO_DESER

        Request._get_deserializer = all_deser.get  # type: ignore [assignment]

    return Request._get_deserializer(constructor_id)


class Request(Generic[Return]):
    __slots__ = ("data", "_target_cid")

    def __init__(self, data: bytes, *, target_cid: Optional[int] = None):
        self.data = bytes(data)
        self._target_cid = target_cid

    def __buffer__(self, flags: int, /) -> memoryview:
        return memoryview(self.data).__buffer__(flags)

    def __release_buffer__(self, buffer: memoryview, /) -> None:
        pass

    @property
    def constructor_id(self) -> int:
        try:
            cid = struct.unpack_from("<I", self.data)[0]
            assert isinstance(cid, int)
            return cid
        except struct.error:
            return 0

    @property
    def target_cid(self) -> int:
        return self._target_cid if self._target_cid is not None else self.constructor_id

    _get_deserializer = staticmethod(_bootstrap_get_deserializer)

    def deserialize_response(self, response: bytes) -> Return:
        deserializer = self._get_deserializer(self.target_cid)
        assert deserializer is not None
        return deserializer(response)  # type: ignore [no-any-return]

    def debug_name(self) -> str:
        return f"request#{self.constructor_id:x}"
