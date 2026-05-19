# {{project_name}}

Single-step research agent: searches the web, fetches the top result, and
returns a summary. Every tool call is captured via `@actantdb/mastra` so you
can replay any decision point in Studio.

## Quick start

```bash
npm install
npm run demo
# In a second terminal:
npm run studio
```

Open http://localhost:{{studio_port}} and click any tool call to see the model
context manifest, the policy verdict, and the result.
