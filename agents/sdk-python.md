# Work package: `sdks/python` — `actantdb`

## Context

Python SDK. Phase 1 alpha command set; grows with each phase via codegen.

## Specs to read first

- `/specs/09-sdk-design.md` §9.
- `/planning/sdk-python.md`.
- `/specs/08-api-spec.md`.

## Scope

### Layout

```
sdks/python/
├── pyproject.toml              (PEP 621; httpx + websockets + pydantic v2)
├── README.md
├── actantdb/
│   ├── __init__.py
│   ├── client.py               (ActantClient async)
│   ├── sync_client.py          (ActantClient.sync façade)
│   ├── transport.py
│   ├── subscribe.py            (async generator)
│   ├── errors.py
│   ├── auth.py
│   ├── _generated.py           (codegen output)
│   └── py.typed
└── tests/
```

### Tests

- pytest-asyncio for async tests.
- Unit + integration (against `actantdb-server` in a CI service container).
- Sync façade round-trip parity with async.
- `mypy --strict` clean.

## Acceptance criteria

- [ ] `python -m build && pip install dist/*.whl` works in a clean venv.
- [ ] `pytest` green.
- [ ] `mypy --strict actantdb` clean.
- [ ] `actantdb` imports without invoking the network.

## Do NOT

- Do NOT add a non-pydantic data layer.
- Do NOT depend on libraries that need C extensions when a pure-Python fallback exists.

## Hand-off

`pytest` and a manual alpha-demo drive from a script.
