---
description: Finish a task by running local gates, CodeRabbit review, a fix-review loop, and PR preparation.
---

// turbo-all

Use this workflow before final completion of code-changing tasks, and when the user asks to finish, ship, run CodeRabbit, start a review loop, or make a PR.

1. Inspect the current branch and dirty tree:
   git status --short --branch

2. Keep all work on the current branch unless the user explicitly approves creating or switching branches. If the current branch is main or master and a PR is required, pause for branch approval before committing. Never stash. Do not discard unrelated edits.

3. Identify the files owned by the current task. If unrelated dirty edits exist, exclude them from staging, commits, and review scope. If the task changes cannot be isolated cleanly, ask before continuing.

4. Run the relevant local gates for the changed surface:
   just verify-specs
   just verify-agents
   pnpm smoke
   pnpm -r build
   cargo check -p <crate> --all-targets

5. If contracts changed, run:
   cargo run -p actant-contracts -- check-compat
   cargo run -p actant-contracts -- codegen-ts

6. If code changed and graphify-out/graph.json exists, run:
   graphify update .

7. Verify CodeRabbit is installed and authenticated:
   coderabbit --version
   coderabbit auth status --agent

8. Run CodeRabbit with repository guidance only after the user has asked for this finish/review/PR workflow. If the task-owned files can be committed without including unrelated dirty edits, commit those files by explicit path first, then review the committed diff:
   coderabbit review --agent -c AGENTS.md -t committed

   If the task-owned files must remain uncommitted, use uncommitted mode with explicit scoping so unrelated dirty edits are not reviewed:
   coderabbit review --agent -c AGENTS.md -t uncommitted

   If the worktree has no uncommitted changes and the branch has local commits to review, also use:
   coderabbit review --agent -c AGENTS.md -t committed

   When using uncommitted mode in a dirty worktree, add CodeRabbit scoping flags such as:
   coderabbit review --agent -c AGENTS.md -t uncommitted --dir .codex

9. Parse CodeRabbit NDJSON findings by severity. Fix every actionable issue in the current branch, then rerun the relevant local gates.

10. Repeat the CodeRabbit review and fix cycle until CodeRabbit raises 0 actionable issues, or until the remaining issues are non-actionable and the user agrees to carry them.

11. Stage files by explicit path, commit the finished work, and prepare a PR summary with:
    - What changed
    - Local gates run
    - CodeRabbit review result

12. Pause for explicit approval before pushing or opening the PR. After approval, push the current branch and create the PR with gh.
