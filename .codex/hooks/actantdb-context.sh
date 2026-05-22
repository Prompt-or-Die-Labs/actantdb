#!/bin/sh

event=${1:-}
payload=$(cat 2>/dev/null || true)
tool_input=${TOOL_INPUT:-}

emit_context() {
  ACTANT_CONTEXT=$1 python3 - <<'PY'
import json
import os

context = os.environ.get("ACTANT_CONTEXT", "")
if context:
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "ActantDB",
            "additionalContext": context,
        }
    }))
PY
}

extract_field() {
  ACTANT_PAYLOAD=$payload ACTANT_TOOL_INPUT=$tool_input ACTANT_FIELD=$1 python3 - <<'PY'
import json
import os

payload = os.environ.get("ACTANT_TOOL_INPUT") or os.environ.get("ACTANT_PAYLOAD") or "{}"
field = os.environ.get("ACTANT_FIELD", "")
try:
    data = json.loads(payload)
except Exception:
    data = {}

tool_input = data.get("tool_input")
if isinstance(tool_input, str):
    try:
        tool_input = json.loads(tool_input)
    except Exception:
        tool_input = {}
if not isinstance(tool_input, dict):
    tool_input = {}

for source in (tool_input, data):
    value = source.get(field)
    if isinstance(value, str):
        print(value)
        raise SystemExit
PY
}

case "$event" in
  session-start)
    extra="Project skills live in .agent/skills: actantdb-codegen, actantdb-graphify, actantdb-verify, and dashboard. Workflows live in .agent/workflows. Use .agent/workflows/finish-with-coderabbit.md before final completion of code-changing goals, and whenever the user asks to finish, ship, review with CodeRabbit, or make a PR. Git hooks live in .githooks and should be active through core.hooksPath=.githooks."
    if [ -f graphify-out/graph.json ]; then
      extra="$extra For codebase questions, run graphify query, graphify path, or graphify explain before broad source search."
    fi
    emit_context "ActantDB setup: read AGENTS.md first. $extra"
    ;;
  prompt-submit)
    prompt=$(extract_field prompt)
    prompt_lc=$(printf '%s' "$prompt" | tr '[:upper:]' '[:lower:]')
    context=""
    case "$prompt_lc" in
      *contract*|*public\ type*|*generated\ type*|*codegen*|*binding*)
        context="Contract changes must start in crates/actant-contracts. Run cargo run -p actant-contracts --bin actant-contracts -- check-compat, then cargo run -p actant-contracts --bin actant-contracts -- codegen-ts. Never hand-edit packages/actant-types/src/generated."
        ;;
      *graphify*|*architecture*|*dependency*|*relationship*|*where\ is*|*how\ does*)
        if [ -f graphify-out/graph.json ]; then
          context="Graphify is available. For codebase questions, query graphify-out with graphify query/path/explain before broad raw-source browsing."
        fi
        ;;
      *verify*|*test*|*green*|*ci*|*smoke*)
        context="Use ActantDB verification gates: just verify-specs, just verify-agents, crate-specific cargo test -p <crate>, pnpm -r build/test, and pnpm smoke. Avoid local cargo test --workspace."
        ;;
      *ready\ to\ finish*|*ready\ to\ ship*|*ship\ it*|*ship\ this*|*make\ pr*|*open\ pr*|*create\ pr*|*close\ pr*|*pull\ request*|*coderabbit*|*code\ rabbit*|*review\ loop*|*finalize*|*complete\ task*|*task\ complete*|*task\ is\ complete*|*goal\ complete*|*goal\ is\ complete*|*all\ done*|*mark\ as\ done*|*work\ is\ done*|*changes\ are\ done*)
        context="Use .agent/workflows/finish-with-coderabbit.md before final completion. Run local gates, run CodeRabbit review with AGENTS.md context, fix every actionable issue, repeat until clean, then pause for approval before push or PR creation."
        ;;
    esac
    emit_context "$context"
    ;;
  pre-tool)
    command=$(extract_field command)
    context=""
    case "$command" in
      *"cargo test --workspace"*|*"cargo test  --workspace"*)
        context="ActantDB local rule: do not run cargo test --workspace. Use cargo test -p <crate> <test_name> or narrower crate gates."
        ;;
      *"git add -A"*|*"git add ."*)
        context="ActantDB git rule: stage files by name. Avoid git add -A and git add . in this repo."
        ;;
      *grep*|*rg\ *|*ripgrep*|*find\ *|*fd\ *|*ack\ *|*ag\ *)
        if [ -f graphify-out/graph.json ]; then
          context="Graphify is available at graphify-out. For focused codebase questions, prefer graphify query/path/explain before broad raw-source search."
        fi
        ;;
    esac
    emit_context "$context"
    ;;
  post-write)
    target=$(extract_field file_path)
    if [ -z "$target" ]; then
      target=$(extract_field TargetFile)
    fi
    context=""
    case "$target" in
      *"crates/actant-contracts/"*)
        context="Contract source changed. Run cargo run -p actant-contracts --bin actant-contracts -- check-compat and cargo run -p actant-contracts --bin actant-contracts -- codegen-ts before finishing."
        ;;
      *"packages/actant-types/src/generated/"*)
        context="Generated bindings are not hand-edited in ActantDB. Change crates/actant-contracts and regenerate instead."
        ;;
    esac
    emit_context "$context"
    ;;
esac
