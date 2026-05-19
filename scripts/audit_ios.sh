#!/usr/bin/env bash
# audit_ios.sh — grep the Rust workspace for iOS-incompatible patterns.
#
# Per docs/IOS_EMBEDDING.md §2: iOS embedding bans process-spawn, system
# libsqlite linkage, hard-coded $HOME paths, native-tls, fork/exec, and
# tempfile without an explicit dir argument. This script greps every
# `crates/*/src/` tree for those patterns and emits a punch list.
#
# Status legend:
#   [ok]   no findings in this category for this crate
#   [warn] findings live in a binary or module that is host-only by gate
#          (`#![cfg(feature = "...")]` or `#[cfg(not(target_os = "ios"))]`)
#   [fail] findings live in library code that the iOS embedder reaches
#
# Output is also captured to docs/IOS_AUDIT.md by the wrapper at
# `actantdb audit-ios` (crates/actant-cli/src/cmd/audit_ios.rs).
#
# Exit code: always 0 — the script's job is to surface findings, not gate
# the build. The CI job `build-ios` in .github/workflows/ci.yml is the gate.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Crates to audit. We list explicitly so a stray top-level file doesn't
# get walked.
CRATES=()
for c in crates/*/; do
    [ -d "${c}src" ] || continue
    CRATES+=("$(basename "$c")")
done

# Common exclusions for every grep.
EXCLUDES=(
    --exclude=audit_ios.sh
    --exclude-dir=target
    --exclude-dir=node_modules
    --exclude-dir=tests          # test code can use Command, tempfile, etc.
    --exclude-dir=examples
    --exclude-dir=benches
    --exclude=GRAPH_REPORT.md
    --exclude=IOS_AUDIT.md
)

# ---------------------------------------------------------------------------
# Pattern definitions. Each entry: NAME|REGEX|RATIONALE
# ---------------------------------------------------------------------------
PATTERNS=(
    "process-spawn|std::process::Command|tokio::process|iOS sandbox forbids spawning arbitrary binaries (posix_spawn to non-bundled bins)."
    "system-libsqlite|libsqlite3-sys[^/]|System libsqlite linkage; iOS must use sqlx with the bundled feature (default)."
    "home-dir|home_dir\(\)|dirs::home_dir|Hard-coded \$HOME path; iOS caller supplies the sandbox dir explicitly."
    "tilde-actantdb|~/\.actantdb|/\.actantdb/|Hard-coded ~/.actantdb path; iOS caller supplies the sandbox dir explicitly."
    "native-tls|native-tls|native_tls|tls-native-tls|Should use rustls-tls on iOS to avoid Security.framework weight."
    "fork-exec|libc::fork|libc::execv|libc::execve|nix::unistd::fork|iOS sandbox forbids fork/exec."
    "process-ids|libc::getpid|libc::getuid|nix::unistd::getpid|nix::unistd::getuid|Process-table reads are sandboxed on iOS; wrap in #[cfg(not(target_os = \"ios\"))]."
    "tempfile-no-dir|tempfile::tempdir\(\)|tempfile::NamedTempFile::new\(\)|tempfile without an explicit dir argument falls back to /tmp; iOS needs the app sandbox tempdir."
)

# Multi-pattern regex helper. `grep -E -e p1 -e p2 …` matches any.
grep_multi() {
    local crate="$1"; shift
    local args=()
    for p in "$@"; do
        args+=(-e "$p")
    done
    LC_ALL=C grep -RnEH "${EXCLUDES[@]}" "${args[@]}" "crates/$crate/src" 2>/dev/null || true
}

# Classify a finding by gate level:
#   - if the file has a `#![cfg(not(target_os = "ios"))]` or a binary
#     `required-features` gate that the workspace disables on iOS, it's
#     a [warn] (host-only).
#   - if the file's parent `mod` declaration in `lib.rs` is gated behind
#     a feature flag that iOS embedders won't enable (shell/file/browser/
#     email/mcp/slack/model/cdp), the module is host-only -> [warn].
#   - otherwise it's a [fail].
classify() {
    local file="$1"
    if [ ! -f "$file" ]; then echo "fail"; return; fi
    # File-level cfg gates the iOS path skips.
    if head -20 "$file" | grep -qE '^#!\[cfg\(not\(target_os = "ios"\)\)\]'; then
        echo "warn"; return
    fi
    # Binary entry points behind a feature flag — these are host-only by
    # convention because the iOS xcframework build doesn't enable those
    # features.
    if head -10 "$file" | grep -qE '^#!\[cfg\(feature ='; then
        echo "warn"; return
    fi
    # Files under src/bin/ are binaries; iOS link is library-only.
    if echo "$file" | grep -q '/src/bin/'; then
        echo "warn"; return
    fi
    # Module re-exports gated behind a feature flag in the parent lib.rs.
    # e.g. `#[cfg(feature = "shell")] pub mod shell;` means `shell.rs`
    # is unreachable from a default iOS build (it doesn't enable the
    # worker features).
    local dir mod_name parent_lib
    dir="$(dirname "$file")"
    mod_name="$(basename "$file" .rs)"
    parent_lib="$(dirname "$dir")/lib.rs"
    # If the file IS a module file inside a sub-directory, walk one more up.
    if [ ! -f "$parent_lib" ]; then
        parent_lib="$dir/../lib.rs"
    fi
    # Try direct sibling lib.rs.
    if [ -f "$dir/lib.rs" ]; then
        parent_lib="$dir/lib.rs"
    fi
    if [ -f "$parent_lib" ]; then
        if grep -qE "^#\[cfg\(feature = \"[a-z_-]+\"\)\][[:space:]]*$" "$parent_lib"; then
            if grep -B1 -E "^pub mod ${mod_name}( |;)" "$parent_lib" \
                 | grep -qE "^#\[cfg\(feature = "; then
                echo "warn"; return
            fi
        fi
    fi
    echo "fail"
}

# ---------------------------------------------------------------------------
# Header
# ---------------------------------------------------------------------------
echo "# ActantDB iOS audit"
echo
echo "Generated by scripts/audit_ios.sh on $(date -u +%Y-%m-%dT%H:%M:%SZ)."
echo "See docs/IOS_EMBEDDING.md §2 for the policy this enforces."
echo
echo "## Pattern legend"
echo
for entry in "${PATTERNS[@]}"; do
    name="${entry%%|*}"
    rest="${entry#*|}"
    rationale="${rest##*|}"
    echo "- **${name}** — ${rationale}"
done
echo
echo "## Status legend"
echo
echo "- \`[ok]\`   no findings in this category for this crate"
echo "- \`[warn]\` findings live in a host-only binary or a feature-gated module the iOS build skips"
echo "- \`[fail]\` findings live in library code the iOS embedder reaches"
echo

TOTAL_OK=0
TOTAL_WARN=0
TOTAL_FAIL=0

# ---------------------------------------------------------------------------
# Per-crate audit
# ---------------------------------------------------------------------------
for crate in "${CRATES[@]}"; do
    echo "## ${crate}"
    echo
    any_finding=0
    for entry in "${PATTERNS[@]}"; do
        name="${entry%%|*}"
        rest="${entry#*|}"
        regex="${rest%%|*}"
        # Allow `|` inside the regex by re-splitting on the LAST pipe:
        # the rationale is everything after the last `|`. Patterns above
        # use `|` as alternation; bash word-split via the format keeps
        # them intact because we used `${entry%%|*}` for name and the
        # rationale is `${rest##*|}` separately.
        IFS='|' read -ra parts <<<"$entry"
        # parts: [name, regex_alts..., rationale]
        nparts=${#parts[@]}
        regex_parts=("${parts[@]:1:$((nparts-2))}")
        # Reassemble alternation regex with literal `|` between parts.
        joined=""
        for rp in "${regex_parts[@]}"; do
            if [ -z "$joined" ]; then joined="$rp"; else joined="${joined}|${rp}"; fi
        done

        # Run the grep.
        hits=$(grep_multi "$crate" "$joined")
        if [ -z "$hits" ]; then
            continue
        fi
        any_finding=1
        # Group by file; classify by file.
        echo "### ${name}"
        echo
        echo '```'
        # Print each hit with a leading status tag.
        prev_file=""
        prev_status=""
        while IFS= read -r line; do
            file="${line%%:*}"
            if [ "$file" != "$prev_file" ]; then
                prev_file="$file"
                prev_status=$(classify "$file")
                case "$prev_status" in
                    warn) TOTAL_WARN=$((TOTAL_WARN+1));;
                    fail) TOTAL_FAIL=$((TOTAL_FAIL+1));;
                esac
            fi
            echo "[${prev_status}] ${line}"
        done <<<"$hits"
        echo '```'
        echo
    done
    if [ "$any_finding" -eq 0 ]; then
        echo "[ok] no iOS-incompatible patterns found."
        echo
        TOTAL_OK=$((TOTAL_OK+1))
    fi
done

echo "## Summary"
echo
echo "- crates clean: ${TOTAL_OK}"
echo "- files with [warn] findings: ${TOTAL_WARN}"
echo "- files with [fail] findings: ${TOTAL_FAIL}"
echo
if [ "$TOTAL_FAIL" -eq 0 ]; then
    echo "All findings are in host-only binaries or feature-gated modules. Safe to flip the CI \`build-ios\` job from \`continue-on-error: true\` to a hard fail."
else
    echo "There are still [fail] findings in library code reached by the iOS embedder. Track each in docs/IOS_AUDIT.md and fix before flipping the CI gate."
fi

exit 0
