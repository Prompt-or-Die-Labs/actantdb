"""Unit tests for the Python client.

These mock the urllib transport — they don't require the server to be
running. There's a separate integration test below that's skipped unless
`ACTANTDB_TEST_URL` is set.
"""

from __future__ import annotations

import io
import json
import os
import unittest
from unittest.mock import patch

from actantdb import ActantClient, ActantError


class FakeResponse(io.BytesIO):
    def __init__(self, body: bytes):
        super().__init__(body)
        self.headers = {}

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False


class ClientTests(unittest.TestCase):
    def test_create_session_posts_command(self):
        captured = {}

        def fake_open(req, timeout):
            captured["url"] = req.full_url
            captured["body"] = json.loads(req.data.decode("utf-8"))
            return FakeResponse(
                json.dumps(
                    {"command_id": "cmd_x", "result": {"session_id": "sess_y"}}
                ).encode("utf-8")
            )

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            sid = c.create_session(workspace_id="ws_1", actor_id="act_1")
        self.assertEqual(sid, "sess_y")
        self.assertEqual(captured["body"]["command_type"], "create_session")

    def test_bearer_token_attached(self):
        captured = {}

        def fake_open(req, timeout):
            captured["auth"] = req.get_header("Authorization")
            return FakeResponse(b"{}")

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555", token="tok_abc")
            c.healthz()
        self.assertEqual(captured["auth"], "Bearer tok_abc")

    def test_http_error_surfaces_as_actant_error(self):
        import urllib.error

        def fake_open(req, timeout):
            raise urllib.error.HTTPError(
                req.full_url,
                400,
                "Bad Request",
                {},
                io.BytesIO(b'{"error":"invalid_input","message":"missing field"}'),
            )

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            with self.assertRaises(ActantError) as cm:
                c.create_session(workspace_id="ws_1", actor_id="act_1")
        self.assertEqual(cm.exception.status, 400)

    def test_rejects_non_http_base_url(self):
        with self.assertRaises(ValueError):
            ActantClient("file:///tmp/actantdb.sock")

    def test_memories_query_includes_status(self):
        captured = {}

        def fake_open(req, timeout):
            captured["url"] = req.full_url
            return FakeResponse(b'{"memories":[]}')

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            c.memories(workspace_id="ws_1", status="pending")
        self.assertIn("status=pending", captured["url"])
        self.assertIn("workspace_id=ws_1", captured["url"])

    def test_grant_permission_body(self):
        captured = {}

        def fake_open(req, timeout):
            captured["url"] = req.full_url
            captured["method"] = req.get_method()
            captured["body"] = json.loads(req.data.decode("utf-8"))
            return FakeResponse(b'{"id":"auth_x"}')

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            r = c.grant_permission(
                workspace_id="ws_1",
                actor_id="act_1",
                permission="file.read",
                level="low",
                scope="~/Projects/**",
                allowed_actions=["read"],
            )
        self.assertEqual(captured["method"], "POST")
        self.assertEqual(captured["body"]["permission"], "file.read")
        self.assertEqual(captured["body"]["allowed_actions"], ["read"])
        self.assertEqual(r["id"], "auth_x")

    def test_revoke_permission_uses_authority_scope_id(self):
        captured = {}

        def fake_open(req, timeout):
            captured["method"] = req.get_method()
            captured["body"] = json.loads(req.data.decode("utf-8"))
            return FakeResponse(b'{"ok":true}')

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            c.revoke_permission(workspace_id="ws_1", authority_scope_id="auth_x")
        self.assertEqual(captured["method"], "DELETE")
        self.assertEqual(captured["body"]["authority_scope_id"], "auth_x")

    def test_save_setup_report_posts_content(self):
        captured = {}

        def fake_open(req, timeout):
            captured["body"] = json.loads(req.data.decode("utf-8"))
            return FakeResponse(b'{"artifact_id":"art_x","event_id":"evt_y"}')

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            r = c.save_setup_report(
                workspace_id="ws_1", actor_id="act_1", content="hello"
            )
        self.assertEqual(captured["body"]["content"], "hello")
        self.assertEqual(r["artifact_id"], "art_x")

    def test_latest_setup_report_query(self):
        captured = {}

        def fake_open(req, timeout):
            captured["url"] = req.full_url
            return FakeResponse(b'{"report":null}')

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            c.latest_setup_report(workspace_id="ws_1")
        self.assertIn("latest=true", captured["url"])

    def test_save_scout_record_body(self):
        captured = {}

        def fake_open(req, timeout):
            captured["body"] = json.loads(req.data.decode("utf-8"))
            return FakeResponse(b'{"artifact_id":"art_x","event_id":"evt_y"}')

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            c.save_scout_record(
                workspace_id="ws_1",
                actor_id="act_1",
                source_id="src_a",
                kind="page",
                sensitivity="low",
                content="payload",
                metadata={"title": "hi"},
            )
        self.assertEqual(captured["body"]["source_id"], "src_a")
        self.assertEqual(captured["body"]["metadata"], {"title": "hi"})

    def test_scout_records_optional_source(self):
        captured = {}

        def fake_open(req, timeout):
            captured["url"] = req.full_url
            return FakeResponse(b'{"records":[]}')

        with patch("urllib.request.urlopen", side_effect=fake_open):
            c = ActantClient("http://x:4555")
            c.scout_records(workspace_id="ws_1", source="src_a")
        self.assertIn("source=src_a", captured["url"])


@unittest.skipUnless(
    os.environ.get("ACTANTDB_TEST_URL"),
    "set ACTANTDB_TEST_URL to a running actantdb-server to enable",
)
class IntegrationTests(unittest.TestCase):
    def test_health(self):
        c = ActantClient(os.environ["ACTANTDB_TEST_URL"])
        h = c.healthz()
        self.assertEqual(h["status"], "ok")


if __name__ == "__main__":
    unittest.main()
