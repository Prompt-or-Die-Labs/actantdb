"""CrewAI adapter helpers for ActantDB."""

from __future__ import annotations

import json
from typing import Any, Dict

from .client import ActantClient


class ActantCrewAITracer:
    """Small tracer object for CrewAI task and tool callbacks."""

    def __init__(
        self,
        client: ActantClient,
        *,
        workspace_id: str,
        actor_id: str,
        source_id: str = "crewai",
        sensitivity: str = "low",
    ) -> None:
        self.client = client
        self.workspace_id = workspace_id
        self.actor_id = actor_id
        self.source_id = source_id
        self.sensitivity = sensitivity

    def before_kickoff(self, inputs: Dict[str, Any]) -> None:
        self.record("kickoff_start", inputs)

    def after_kickoff(self, result: Any) -> None:
        self.record("kickoff_end", {"result": _jsonable(result)})

    def on_tool_start(self, tool_name: str, arguments: Dict[str, Any]) -> None:
        self.record("tool_start", {"tool_name": tool_name, "arguments": arguments})

    def on_tool_end(self, tool_name: str, result: Any) -> None:
        self.record("tool_end", {"tool_name": tool_name, "result": _jsonable(result)})

    def record(self, kind: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        return self.client.save_scout_record(
            workspace_id=self.workspace_id,
            actor_id=self.actor_id,
            source_id=self.source_id,
            kind=kind,
            sensitivity=self.sensitivity,
            content=json.dumps(payload, sort_keys=True, separators=(",", ":")),
            metadata={"adapter": "crewai"},
        )


def _jsonable(value: Any) -> Any:
    try:
        json.dumps(value)
        return value
    except TypeError:
        return repr(value)
