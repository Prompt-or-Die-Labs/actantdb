#!/usr/bin/env bash
# Materialize the mdbook source from the repo's canonical specs + docs.
# Run before `mdbook build`.

set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
out="$here/src"

mkdir -p "$out/specs" "$out/adr"

# Copy canonical specs (rewrite .sql → .md so mdbook picks them up).
for f in "$repo/specs"/*.md; do
  base="$(basename "$f")"
  cp "$f" "$out/specs/$base"
done

# Copy ADRs.
for f in "$repo/specs/adr"/*.md; do
  base="$(basename "$f")"
  cp "$f" "$out/adr/$base"
done

# Spec 02 is .sql; render it inside a markdown fence so it's browsable.
{
  echo "# 02 — Data model"
  echo
  echo "Canonical schema. Source of truth for every \`CREATE TABLE\`. See"
  echo "\`/migrations/0001_initial.sql\` for the runnable Phase-1 migration."
  echo
  echo '```sql'
  cat "$repo/specs/02-data-model.sql"
  echo '```'
} > "$out/specs/02-data-model.md"

# Top-level docs.
cp "$repo/CHANGELOG.md" "$out/CHANGELOG.md"
cp "$repo/SPECS_STATUS.md" "$out/SPECS_STATUS.md"
cp "$repo/GATES.md" "$out/GATES.md" 2>/dev/null || true
cp "$repo/RELEASE_CHECKLIST.md" "$out/RELEASE_CHECKLIST.md" 2>/dev/null || true
cp "$here/SLO.md" "$out/SLO.md"

echo "docs/src/ rebuilt from $repo"
