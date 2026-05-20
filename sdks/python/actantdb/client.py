"""HTTP client for the ActantDB server."""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Dict, Iterable, Optional

_ALLOWED_URL_SCHEMES = {"http", "https"}


class ActantError(RuntimeError):
    """Raised for any non-2xx HTTP response from the server."""

    def __init__(self, status: int, body: str):
        super().__init__(f"HTTP {status}: {body}")
        self.status = status
        self.body = body


def _validate_url_scheme(url: str) -> None:
    scheme = urllib.parse.urlparse(url).scheme
    if scheme not in _ALLOWED_URL_SCHEMES:
        allowed = ", ".join(sorted(_ALLOWED_URL_SCHEMES))
        raise ValueError(f"ActantClient only supports these URL schemes: {allowed}")


def _build_http_opener() -> urllib.request.OpenerDirector:
    opener = urllib.request.OpenerDirector()
    opener.add_handler(urllib.request.HTTPHandler())
    opener.add_handler(urllib.request.HTTPSHandler())
    opener.add_handler(urllib.request.HTTPDefaultErrorHandler())
    opener.add_handler(urllib.request.HTTPRedirectHandler())
    opener.add_handler(urllib.request.HTTPErrorProcessor())
    return opener


_HTTP_OPENER = _build_http_opener()


def _open_http_request(req: urllib.request.Request, timeout: float):
    _validate_url_scheme(req.full_url)
    return _HTTP_OPENER.open(req, timeout=timeout)


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
        _validate_url_scheme(self.base_url)
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
        _validate_url_scheme(url)
        data = None
        if body is not None:
            data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(url, data=data, headers=self._headers(), method=method)
        try:
            with _open_http_request(req, self.timeout) as resp:
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

    # ------------------------------------------------------------------
    # Memories

    def memories(
        self,
        *,
        workspace_id: str,
        status: str = "approved",
    ) -> Dict[str, Any]:
        return self._request(
            "GET",
            "/v1/memories",
            params=[("workspace_id", workspace_id), ("status", status)],
        )

    def memory_conflicts(self, *, workspace_id: str) -> Dict[str, Any]:
        return self._request(
            "GET",
            "/v1/memories/conflicts",
            params=[("workspace_id", workspace_id)],
        )

    # ------------------------------------------------------------------
    # Permissions

    def permissions(self, *, workspace_id: str) -> Dict[str, Any]:
        return self._request(
            "GET", "/v1/permissions", params=[("workspace_id", workspace_id)]
        )

    def grant_permission(
        self,
        *,
        workspace_id: str,
        actor_id: str,
        permission: str,
        level: str,
        scope: Optional[str] = None,
        allowed_actions: Optional[Iterable[str]] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {
            "workspace_id": workspace_id,
            "actor_id": actor_id,
            "permission": permission,
            "level": level,
        }
        if scope is not None:
            body["scope"] = scope
        if allowed_actions is not None:
            body["allowed_actions"] = list(allowed_actions)
        return self._request("POST", "/v1/permissions", body=body)

    def revoke_permission(
        self, *, workspace_id: str, authority_scope_id: str
    ) -> Dict[str, Any]:
        return self._request(
            "DELETE",
            "/v1/permissions",
            body={"workspace_id": workspace_id, "authority_scope_id": authority_scope_id},
        )

    # ------------------------------------------------------------------
    # Setup reports

    def save_setup_report(
        self, *, workspace_id: str, actor_id: str, content: str
    ) -> Dict[str, Any]:
        return self._request(
            "POST",
            "/v1/setup-reports",
            body={
                "workspace_id": workspace_id,
                "actor_id": actor_id,
                "content": content,
            },
        )

    def latest_setup_report(self, *, workspace_id: str) -> Dict[str, Any]:
        return self._request(
            "GET",
            "/v1/setup-reports",
            params=[("workspace_id", workspace_id), ("latest", "true")],
        )

    def setup_reports(self, *, workspace_id: str) -> Dict[str, Any]:
        return self._request(
            "GET",
            "/v1/setup-reports",
            params=[("workspace_id", workspace_id)],
        )

    # ------------------------------------------------------------------
    # Scout records

    def save_scout_record(
        self,
        *,
        workspace_id: str,
        actor_id: str,
        source_id: str,
        kind: str,
        sensitivity: str,
        content: str,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {
            "workspace_id": workspace_id,
            "actor_id": actor_id,
            "source_id": source_id,
            "kind": kind,
            "sensitivity": sensitivity,
            "content": content,
        }
        if metadata is not None:
            body["metadata"] = metadata
        return self._request("POST", "/v1/scout-records", body=body)

    def scout_records(
        self, *, workspace_id: str, source: Optional[str] = None
    ) -> Dict[str, Any]:
        params: list[tuple[str, str]] = [("workspace_id", workspace_id)]
        if source is not None:
            params.append(("source", source))
        return self._request("GET", "/v1/scout-records", params=params)
