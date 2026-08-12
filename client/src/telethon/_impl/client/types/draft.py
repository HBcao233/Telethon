from __future__ import annotations

import datetime
from typing import TYPE_CHECKING, Self

from ...session import PeerRef
from ...tl import abcs, functions, types
from ..parsers import generate_html_message, generate_markdown_message
from .message import Message, generate_random_id
from .meta import NoPublicConstructor
from .peer import Peer, PeerMap, expand_peer, peer_id

if TYPE_CHECKING:
    from ..client.client import Client


class Draft(metaclass=NoPublicConstructor):
    """
    A draft message in a chat.

    You can obtain drafts with methods such as :meth:`telethon.Client.get_drafts`.
    """

    def __init__(
        self,
        client: Client,
        peer: abcs.Peer,
        top_msg_id: int | None,
        raw: abcs.DraftMessage,
        chat_map: PeerMap,
    ) -> None:
        assert isinstance(raw, (types.DraftMessage, types.DraftMessageEmpty))
        self._client = client
        self._peer = peer
        self._raw = raw
        self._top_msg_id = top_msg_id
        self._chat_map = chat_map

    @classmethod
    def _from_raw_update(
        cls,
        client: Client,
        draft: types.UpdateDraftMessage,
        chat_map: PeerMap,
    ) -> Self:
        return cls._create(client, draft.peer, draft.top_msg_id, draft.draft, chat_map)

    @classmethod
    def _from_raw(
        cls,
        client: Client,
        peer: abcs.Peer,
        top_msg_id: int,
        draft: abcs.DraftMessage,
        chat_map: PeerMap,
    ) -> Self:
        return cls._create(client, peer, top_msg_id, draft, chat_map)

    @property
    def chat(self) -> Peer:
        """
        The chat where the draft is saved.

        This is also the chat where the message will be sent to by :meth:`send`.
        """
        return self._chat_map.get(peer_id(self._peer)) or expand_peer(
            self._client, self._peer, broadcast=None
        )

    @property
    def link_preview(self) -> bool:
        """
        :data:`True` if the link preview is allowed to exist when sending the message.
        """
        return not getattr(self._raw, "no_webpage", False)

    @property
    def replied_message_id(self) -> int | None:
        """
        Get the message identifier of message this draft will reply to once sent.
        """
        return self._raw.reply_to_msg_id or None

    @property
    def text(self) -> str | None:
        """
        The :attr:`~Message.text` of the message that will be sent.
        """
        return getattr(self._raw, "message", None)

    @property
    def text_html(self) -> str | None:
        """
        The :attr:`~Message.text_html` of the message that will be sent.
        """
        if text := getattr(self._raw, "message", None):
            return generate_html_message(
                text, getattr(self._raw, "entities", None) or []
            )
        else:
            return None

    @property
    def text_markdown(self) -> str | None:
        """
        The :attr:`~Message.text_markdown` of the message that will be sent.
        """
        if text := getattr(self._raw, "message", None):
            return generate_markdown_message(
                text, getattr(self._raw, "entities", None) or []
            )
        else:
            return None

    @property
    def date(self) -> datetime.datetime | None:
        """
        The date when the draft was last updated.
        """
        date = getattr(self._raw, "date", None)
        return (
            datetime.datetime.fromtimestamp(date, tz=datetime.UTC)
            if date is not None
            else None
        )

    async def edit(
        self,
        text: str | None = None,
        *,
        markdown: str | None = None,
        html: str | None = None,
        link_preview: bool = False,
        reply_to: int | None = None,
    ) -> Draft:
        """
        Replace the current draft with a new one.

        :param text: See :ref:`formatting`.
        :param markdown: See :ref:`formatting`.
        :param html: See :ref:`formatting`.
        :param link_preview: See :ref:`formatting`.

        :param reply_to:
            The message identifier of the message to reply to.

        :return: The edited draft.

        .. rubric:: Example

        .. code-block:: python

            new_draft = await old_draft.edit('new text', link_preview=False)
        """
        return await self._client.edit_draft(
            self._peer_ref(),
            text,
            markdown=markdown,
            html=html,
            link_preview=link_preview,
            reply_to=reply_to,
        )

    def _peer_ref(self) -> PeerRef:
        if chat := self._chat_map.get(peer_id(self._peer)):
            return chat._ref
        else:
            return PeerRef._empty_from_peer(self._peer)

    async def send(self) -> Message:
        """
        Send the contents of this draft to the chat.

        The draft will be cleared after being sent.

        :return: The sent message.

        .. rubric:: Example

        .. code-block:: python

            await draft.send(clear=False)
        """
        reply_to = self.replied_message_id
        message = getattr(self._raw, "message", "")
        entities = getattr(self._raw, "entities", None)
        random_id = generate_random_id()

        result = await self._client(
            functions.messages.send_message(
                no_webpage=not self.link_preview,
                silent=False,
                background=False,
                clear_draft=True,
                noforwards=False,
                update_stickersets_order=False,
                peer=self._peer_ref()._to_input_peer(),
                reply_to=(
                    types.InputReplyToMessage(reply_to_msg_id=reply_to, top_msg_id=None)
                    if reply_to
                    else None
                ),
                message=message,
                random_id=random_id,
                reply_markup=None,
                entities=entities,
                schedule_date=None,
                send_as=None,
            )
        )
        if isinstance(result, types.UpdateShortSentMessage):
            return Message._from_defaults(
                self._client,
                {},
                out=result.out,
                id=result.id,
                from_id=(
                    types.PeerUser(user_id=self._client.me.id.bare_id)
                    if self._client.me
                    else None
                ),
                peer_id=self._peer_ref()._to_peer(),
                reply_to=(
                    types.MessageReplyHeader(
                        reply_to_scheduled=False,
                        forum_topic=False,
                        reply_to_msg_id=reply_to,
                        reply_to_peer_id=None,
                        reply_to_top_id=None,
                    )
                    if reply_to
                    else None
                ),
                date=result.date,
                message=message,
                media=result.media,
                entities=result.entities,
                ttl_period=result.ttl_period,
            )
        else:
            return self._client._build_message_map(
                result, self._peer_ref()
            ).with_random_id(random_id)

    async def delete(self) -> None:
        """
        Clear the contents of this draft to delete it.
        """
        await self.edit("")
