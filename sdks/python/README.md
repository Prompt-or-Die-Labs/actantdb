# actantdb (Python SDK)

```bash
pip install -e sdks/python
```

```python
from actantdb import ActantClient

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
