# actantdb (Python SDK)

```bash
pip install -e sdks/python
```

```python
from actantdb import ActantClient, AsyncActantClient

c = ActantClient("http://127.0.0.1:4555")
print(c.healthz())

session_id = c.create_session(workspace_id="ws_default", actor_id="act_system")
c.append_user_message(
    workspace_id="ws_default",
    actor_id="act_system",
    session_id=session_id,
    text="Fix the failing tests.",
)
```

Mirrors the TypeScript [`@actantdb/sdk`](../../packages/actant-sdk/README.md).

Async usage stays dependency-free:

```python
import asyncio
from actantdb import AsyncActantClient

async def main():
    c = AsyncActantClient("http://127.0.0.1:4555")
    print(await c.healthz())

asyncio.run(main())
```

Named Python adapter helpers are included without taking framework
dependencies:

```python
from actantdb import ActantCallbackHandler, ActantCrewAITracer, ActantAutoGenLogger
```

HTTP errors raise `ActantError` with `status`, `code`, `message`, `hint`, and
`fix` fields when the server returns the public error body.
