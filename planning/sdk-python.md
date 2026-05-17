# SDK design — Python

Package: `actantdb`. Phase 1 ships full surface for the alpha command set.

## Tech

- Python 3.10+.
- Async-first (`httpx` + `websockets`).
- Sync façade (`ActantClient.sync`) for scripts.
- Pydantic v2 models for inputs/outputs.
- Ships `py.typed`.

## API

```python
import asyncio
import os
from actantdb import ActantClient

async def main():
    async with ActantClient(
        base_url="https://actant.example.com",
        token=os.environ["ACTANT_TOKEN"],
    ) as client:
        session = await client.command.create_session(
            agent_actor_id="agent_123",
            title="Fix failing tests",
        )
        async for event in client.subscribe("approval_request", status="pending"):
            print(event)

asyncio.run(main())
```

## Distribution

- Published to PyPI as `actantdb`.
- Source under `sdks/python/`.
- Generated code under `sdks/python/actantdb/_generated.py` — never hand-edited.
- Codegen from `actant-sdk-codegen --target py --out sdks/python/actantdb/_generated.py`.

## Conventions

- `snake_case` method and field names (Python idiomatic; the codegen translates from JSON schema's camelCase).
- `Result | raises` model: errors raise `ActantCommandError`.
- `from_env()` constructor reads `ACTANT_SERVER_URL` + `ACTANT_TOKEN`.

## Versioning

Aligned with the TypeScript SDK; same schema major.
