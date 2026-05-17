#!/usr/bin/env python3
"""Minimal MCP-flavored JSON-RPC 2.0 echo server over stdio.

Used by actant-worker-mcp integration tests. Reads newline-delimited
JSON-RPC frames from stdin, responds to:
  - initialize → { "protocolVersion": "2024-11-05", "serverInfo": {...} }
  - tools/call → echoes back { "name": ..., "arguments": ..., "stub": true }

Exits when stdin closes.
"""
import json
import sys


def respond(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def main():
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError:
            continue
        method = req.get("method")
        rid = req.get("id")
        if method == "initialize":
            respond({
                "jsonrpc": "2.0",
                "id": rid,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {"name": "echo-mcp-fixture", "version": "0.0.1"},
                    "capabilities": {"tools": {}},
                },
            })
        elif method == "tools/call":
            params = req.get("params", {})
            respond({
                "jsonrpc": "2.0",
                "id": rid,
                "result": {
                    "name": params.get("name"),
                    "arguments": params.get("arguments"),
                    "echo": True,
                },
            })
        else:
            respond({
                "jsonrpc": "2.0",
                "id": rid,
                "error": {"code": -32601, "message": f"method not found: {method}"},
            })


if __name__ == "__main__":
    main()
