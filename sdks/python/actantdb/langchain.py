"""LangChain callback adapter for ActantDB."""

from __future__ import annotations

import json
from typing import Any, Dict, Optional

from .client import ActantClient


class ActantCallbackHandler:
    """Dependency-free LangChain callback handler.

    LangChain discovers callback methods by name, so this class intentionally
    avoids importing LangChain. Pass an ``ActantClient`` and attach the handler
    anywhere LangChain accepts callbacks.
    """

    def __init__(
        self,
        client: ActantClient,
        *,
        workspace_id: str,
        actor_id: str,
        source_id: str = "langchain",
        sensitivity: str = "low",
    ) -> None:
        self.client = client
        self.workspace_id = workspace_id
        self.actor_id = actor_id
        self.source_id = source_id
        self.sensitivity = sensitivity

    def on_chain_start(self, serialized: Dict[str, Any], inputs: Dict[str, Any], **kw: Any) -> None:
        self._record("chain_start", {"serialized": serialized, "inputs": inputs, "kw": kw})

    def on_chain_end(self, outputs: Dict[str, Any], **kw: Any) -> None:
        self._record("chain_end", {"outputs": outputs, "kw": kw})

    def on_tool_start(self, serialized: Dict[str, Any], input_str: str, **kw: Any) -> None:
        self._record("tool_start", {"serialized": serialized, "input": input_str, "kw": kw})

    def on_tool_end(self, output: str, **kw: Any) -> None:
        self._record("tool_end", {"output": output, "kw": kw})

    def on_tool_error(self, error: BaseException, **kw: Any) -> None:
        self._record("tool_error", {"error": str(error), "kw": kw})

    def on_llm_start(self, serialized: Dict[str, Any], prompts: list[str], **kw: Any) -> None:
        self._record("llm_start", {"serialized": serialized, "prompts": prompts, "kw": kw})

    def on_llm_end(self, response: Any, **kw: Any) -> None:
        self._record("llm_end", {"response": _jsonable(response), "kw": kw})

    def on_llm_error(self, error: BaseException, **kw: Any) -> None:
        self._record("llm_error", {"error": str(error), "kw": kw})

    def _record(self, kind: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        return self.client.save_scout_record(
            workspace_id=self.workspace_id,
            actor_id=self.actor_id,
            source_id=self.source_id,
            kind=kind,
            sensitivity=self.sensitivity,
            content=json.dumps(payload, sort_keys=True, separators=(",", ":")),
            metadata={"adapter": "langchain"},
        )


def _jsonable(value: Any) -> Any:
    try:
        json.dumps(value)
        return value
    except TypeError:
        return repr(value)
