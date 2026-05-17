"""HTTP client for the ActantDB server."""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Dict, Iterable, Optional


class ActantError(RuntimeError):
    """Raised for any non-2xx HTTP response from the server."""

    def __init__(self, status: int, body: str):
        super().__init__(f"HTTP {status}: {body}")
        self.status = status
        self.body = body


class ActantClient:
    """A thin client for /v1/* endpoints.

    Mirrors @actantdb/sdk (TypeScript). Methods are blocking; use the async
    variant in a thread pool if you need concurrency.
    """

    def __init__(
        self,
        base_url: str,
        *,
        token: Optional[str] = None,
        timeout: float = 10.0,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.timeout = timeout

    # ------------------------------------------------------------------
    # Low-level

    def _headers(self) -> Dict[str, str]:
        h = {"content-type": "application/json"}
        if self.token:
            h["authorization"] = f"Bearer {self.token}"
        return h

    def _request(
        self,
        method: str,
        path: str,
        *,
        body: Optional[Dict[str, Any]] = None,
        params: Optional[Iterable[tuple[str, str]]] = None,
    ) -> Dict[str, Any]:
        url = self.base_url + path
        if params:
            url += "?" + urllib.parse.urlencode(list(params))
        data = None
        if body is not None:
            data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(url, data=data, headers=self._headers(), method=method)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read().decode("utf-8")
                return json.loads(raw) if raw else {}
        except urllib.error.HTTPError as e:
            body_text = e.read().decode("utf-8", errors="replace")
            raise ActantError(e.code, body_text) from e

    # ------------------------------------------------------------------
    # Endpoints

    def healthz(self) -> Dict[str, Any]:
        return self._request("GET", "/v1/healthz")

    def metadata_commands(self) -> Dict[str, Any]:
        return self._request("GET", "/v1/metadata/commands")

    def command(
        self,
        *,
        workspace_id: str,
        actor_id: str,
        command_type: str,
        input: Dict[str, Any],
        idempotency_key: Optional[str] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {
            "workspace_id": workspace_id,
            "actor_id": actor_id,
            "command_type": command_type,
            "input": input,
        }
        if idempotency_key is not None:
            body["idempotency_key"] = idempotency_key
        return self._request("POST", "/v1/command", body=body)

    # Alpha-command convenience methods.

    def create_session(
        self, *, workspace_id: str, actor_id: str, title: Optional[str] = None
    ) -> str:
        inp: Dict[str, Any] = {}
        if title is not None:
            inp["title"] = title
        r = self.command(
            workspace_id=workspace_id,
            actor_id=actor_id,
            command_type="create_session",
            input=inp,
        )
        return r["result"]["session_id"]

    def append_user_message(
        self, *, workspace_id: str, actor_id: str, session_id: str, text: str
    ) -> Dict[str, Any]:
        return self.command(
            workspace_id=workspace_id,
            actor_id=actor_id,
            command_type="append_user_message",
            input={"session_id": session_id, "text": text},
        )

    def request_tool_call(
        self,
        *,
        workspace_id: str,
        actor_id: str,
        session_id: str,
        tool_name: str,
        arguments: Dict[str, Any],
    ) -> Dict[str, Any]:
        r = self.command(
            workspace_id=workspace_id,
            actor_id=actor_id,
            command_type="request_tool_call",
            input={
                "session_id": session_id,
                "tool_name": tool_name,
                "arguments": arguments,
            },
        )
        return r["result"]

    def approve_tool_call(
        self,
        *,
        workspace_id: str,
        actor_id: str,
        tool_call_id: str,
        scope: str = "once",
    ) -> Dict[str, Any]:
        return self.command(
            workspace_id=workspace_id,
            actor_id=actor_id,
            command_type="approve_tool_call",
            input={"tool_call_id": tool_call_id, "scope": scope},
        )

    def record_tool_result(
        self,
        *,
        workspace_id: str,
        actor_id: str,
        tool_call_id: str,
        result: Dict[str, Any],
    ) -> Dict[str, Any]:
        return self.command(
            workspace_id=workspace_id,
            actor_id=actor_id,
            command_type="record_tool_result",
            input={"tool_call_id": tool_call_id, "result": result},
        )

    def events(self, *, session_id: str) -> Dict[str, Any]:
        return self._request("GET", "/v1/events", params=[("session_id", session_id)])

    def approvals(self, *, workspace_id: str) -> Dict[str, Any]:
        return self._request(
            "GET", "/v1/approvals", params=[("workspace_id", workspace_id)]
        )
