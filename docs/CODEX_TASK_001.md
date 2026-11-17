# NYRA-001 — Codex First Executable Task

**Priority:** P0
**Current state:** not started / unverified.
**Goal:** create first small Rust CLI slice with shared registry, a native TCP diagnostic, tests and credible environment feedback. Do not implement entire v1 in one PR.

## Inputs
Read: AGENTS.md, docs/CODEX_START_HERE.md, docs/decisions.md, docs/product-requirements.md, docs/architecture.md, docs/tool-catalog.md and docs/testing-release.md.

## Deliverables
1. Valid Cargo binary project named \`nyra\`.
2. CLI commands: \`nyra --help\`, \`nyra tools list\`, \`nyra net tcp <host> <port> --timeout <seconds> --output text|json\`.
3. Central Tool Registry and typed argument validator.
4. Native TCP connect checker with timeout, clear refusal/timeout/invalid target reporting.
5. Structured serializable result with stable, versioned JSON.
6. Unit + integration tests for argument validation, result formatting, success and failures using local deterministic listeners when possible.
7. Tiny README build/run update based on **actual** commands.
8. CI workflow for fmt, clippy and test if runner available.
9. Make and explain any proposed changes to the planned CLI/exit contract; do not silently diverge.

## Acceptance criteria
- Runs without Nmap or external network utility.
- Does not require privilege elevation or persistent host changes.
- Clean stdout for JSON; errors on stderr; no fake timings.
- Uses valid port range; bounded timeout and addresses.
- Program has no hidden AI/network backend dependencies.
- Cargo fmt/clippy/test results supplied; if unavailable, explicitly say so.
- Reviewed commit/PR small enough to inspect independently.

## Suggested Codex prompt

Implement NYRA-001 exactly as scoped in docs/CODEX_TASK_001.md. Read AGENTS.md and related specifications first. Create a minimal Rust/Clap CLI + tool registry + native TCP checker with robust validation, structured JSON and tests. No TUI polish, AI integration, remote connections or optional sound yet. Run real validation commands and report evidence; keep existing project requirements intact and update project status honestly. Prefer a small reviewable PR. 
