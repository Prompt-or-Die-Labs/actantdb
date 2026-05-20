"""AutoGen adapter helpers for ActantDB."""

from __future__ import annotations

import json
from typing import Any, Dict, Optional

from .client import ActantClient


class ActantAutoGenLogger:
    """Message logger for Microsoft AutoGen-style agent events."""

    def __init__(
        self,
        client: ActantClient,
        *,
        workspace_id: str,
        actor_id: str,
        source_id: str = "autogen",
        sensitivity: str = "low",
    ) -> None:
        self.client = client
        self.workspace_id = workspace_id
        self.actor_id = actor_id
        self.source_id = source_id
        self.sensitivity = sensitivity

    def log_message(
        self,
        sender: str,
        recipient: str,
        content: Any,
        *,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        payload = {
            "sender": sender,
            "recipient": recipient,
            "content": _jsonable(content),
            "metadata": metadata or {},
        }
        return self.client.save_scout_record(
            workspace_id=self.workspace_id,
            actor_id=self.actor_id,
            source_id=self.source_id,
            kind="message",
            sensitivity=self.sensitivity,
            content=json.dumps(payload, sort_keys=True, separators=(",", ":")),
            metadata={"adapter": "autogen"},
        )


def _jsonable(value: Any) -> Any:
    try:
        json.dumps(value)
        return value
    except TypeError:
        return repr(value)
