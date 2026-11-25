# Codex Cloud — NYRA Handoff

## Do this first
The first TCP CLI slice passes local Windows x64 GNU checks. Consult the roadmap for remaining Phase 1 work. Read AGENTS.md and the linked documents before implementation.

**Product:** NYRA (name selected after PocketOps planning). It is NOT primarily an AI agent. It is an offline-capable personal systems administration toolkit: Rust binary + human-friendly CLI/TUI + diagnostic workflows + discoverable command reference. Capri is visual/audio inspiration only.

## Agreed requirements to preserve
- Use **nyra** as the CLI/binary name; offer \`nyra\` interactive, \`nyra net tcp ...\` direct, \`nyra workflow run ...\` workflow.
- Native Rust diagnostics first; optional system/external/remote adapters with capability checks.
- 4 built-in editable cyberpunk themes and custom TOML themes, switchable at runtime (F9 proposed).
- Startup/feedback animation and optional event sounds; ability to mute; no mandatory audio dependencies for headless use.
- USB portable / GitHub Releases with separate Windows/Linux binaries and platform matrix tests.
- English task-oriented search, favorites, history/privacy controls, command knowledge/recipes.
- Risk-aware execution, least privilege, no plaintext credentials, robust CLI contracts.
- Agentless SSH for Linux/Solaris later, Windows remote operations and database health beyond basic network check later.
- Documentation + tests + release process must remain reproducible.

## First implementation task — Phase 1 vertical slice
1. Validate/install Rust stable toolchain in Codex environment as allowed.
2. Create a minimal Cargo project producing binary named \`nyra\`; add Clap, a clean module layout, Tool trait/model/registry and typed result.
3. Implement \`nyra --help\`, \`nyra tools list\`, and one **native TCP connect** tool: \`nyra net tcp <host> <port> --timeout <seconds> --output text|json\`.
4. Define/validate input limits, bounded timeout, structured success/failure semantics, stderr/stdout and explicit exit codes.
5. Write unit and integration tests for TCP open/refused/timeout/invalid address where deterministically possible.
6. Add one non-blocking TUI skeleton with a tool list / details mock where practical, but do not delay validated CLI to build animations first.
7. Run format, lint and tests and report actual results; update docs/roadmap.md and README only for proven progress.
8. Commit in small logical changes and open PR where Codex flow expects review. Do not claim v0.1 completed prematurely.

## Suggested work tickets
- NYRA-001: Cargo scaffold, CLI, clean shutdown and CI.
- NYRA-002: Tool registry + execution context/result/error contract.
- NYRA-003: Native TCP port check + tests.
- NYRA-004: Ratatui shell + resize/focus/search UX.
- NYRA-005: Add 11 diagnostic tools with platform/capability checks.
- NYRA-006: Themes + settings + custom TOML.
- NYRA-007: Search/aliases + favorites/history privacy.
- NYRA-008: Workflow engine + Web Diagnostics.
- NYRA-009: Portable packaging/clean-machine validation.
- NYRA-010: Remote checks/incident mode/security/release audit.

## Output expectations
On each change provide: what was implemented; files changed; commands/tests actually run; known limitations; which requirements remain unmet; next smallest ticket. Avoid giant code drops and undocumented dependencies.

## Navigation
- docs/product-requirements.md — complete scope and acceptance
- docs/architecture.md — modules/contracts/data formats
- docs/tui-ux.md — UX and key navigation
- docs/tool-catalog.md — detailed catalog
- docs/experience.md — themes/sounds
- docs/workflows.md — workflow/incident engine
- docs/portability-security.md — deploy/security constraints
- docs/testing-release.md — CI/acceptance/release
- docs/roadmap.md — phases and prioritization
- docs/agent-skills.md — skill selection with verification caveats
