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
