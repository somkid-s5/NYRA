# NYRA — Product Requirements Document (PRD v1.0 planning)

## Problem and audience
A System Administrator needs to inspect networks, hosts, applications, services, certificates and databases on various machines. Installed utilities differ; commands are forgotten; searching an AI/chat/web each time is slow. NYRA should be a personal toolbox that can be downloaded or launched from USB with no per-machine Rust/Python installation.

Primary user: IT/System Administrator, comfortable with Windows/Linux and terminal workflows, but seeking a **faster and more guided** diagnostic experience. Support beginner-friendly descriptions without sacrificing expert CLI speed.

## Product goals
G1. Start diagnostics from a single portable executable on supported hosts.
G2. Find tools by describing the goal in English, even without knowing tool names.
G3. Interactive TUI helps users configure/run/understand tools; direct CLI remains automation-friendly.
G4. Combine checks in safe multi-step diagnostic workflows and export actionable reports.
G5. Be offline-first, extensible, themeable, accessible and safe for production read-only checks.
G6. Deliver reliable binaries, documented limitations, and reproducible builds/tests.

## Functional requirement IDs (must become tickets/tests)
| ID | Priority | Requirement / acceptance |
|---|---|---|
| FR-001 | P0 | \`nyra\` starts interactive TUI in a compatible terminal, with clean teardown |
| FR-002 | P0 | \`nyra --help\`, \`nyra tools list\`, nested CLI subcommands and useful validation errors |
| FR-003 | P0 | One registry describes tool ID, inputs, labels, aliases, OS, risks, deps and outputs |
| FR-004 | P0 | User selects a tool, fills validated inputs, starts/cancels it and reads structured results |
| FR-005 | P0 | At least 12 basic diagnostics spanning network/system/security with tests |
| FR-006 | P0 | Native TCP/DNS/HTTP/TLS functionality does not require Nmap |
| FR-007 | P0 | CLI text and JSON output, stable schema, stdout/stderr and documented exit codes |
| FR-008 | P0 | Search by tool name, curated English aliases and typo-tolerant matching |
| FR-009 | P0 | Windows and Linux release targets tested on clean environments without Rust installed |
| FR-010 | P1 | Four builtin cyberpunk themes, custom TOML palette, live switching and persistence |
| FR-011 | P1 | Partial: the Running screen has a bounded, indeterminate activity marker; F9 controls and persists animation, reduced-motion, and slow/normal/fast speed settings, with static SSH defaults and a skip-on-input startup spinner. Selection effects and optional sound, volume, and event controls remain open; CLI stays quiet. |
| FR-012 | P1 | Favorites and recent tools with privacy controls. In Saved Tools, Ctrl+R disables recent tracking, clears stored recent tool IDs and persists `[discovery].record_recent = false`; enabling it again starts with an empty list. Store tool IDs only, never targets or input values. |
| FR-013 | P1 | Offline command knowledge cards: syntax, description, OS, examples, risk and copy |
| FR-014 | P1 | Partial: typed custom TOML workflows can be validated and run sequentially through the shared executor, with typed inputs/bindings, bounded prior-step conditions, timeout and cancellation. TUI workflow builder, parallel execution, and additional templates remain open. |
| FR-015 | P1 | Direct diagnostics, Web Diagnostics, and custom workflow runs can export sanitized JSON, HTML, or CSV to explicit paths in the CLI. The TUI exports the displayed diagnostic result. Running and exporting custom workflow files in the TUI remain open. |
| FR-016 | P1 | Portable workspace mode keeps config/profile/recipes/history alongside app if writable |
| FR-017 | P1 | Search/help/settings/favorites/workflows and keybindings consistently navigable |
| FR-018 | P2 | Agentless SSH remote checks for authorized Linux/Solaris systems |
| FR-019 | P2 | Windows remote adapter; transport/auth support must be verified |
| FR-020 | P2 | Oracle/MSSQL health check adapters requiring explicit rights where relevant |
| FR-021 | P2 | Incident-mode rules that recommend next checks using evidence, not invented facts |
| FR-022 | P2 | External tools like Nmap detected as optional; missing tools never crash the app |
| FR-023 | Post-v1 | Safe user-defined tool packs, advanced workflows, optional AI/MCP integrations |

## Main screen inventory
Home, global search, tool explorer/category list, tool detail/input form, progress/result, workflows/library/runner, command reference, history/reports, target profiles, settings/appearance/sound and help. See docs/tui-ux.md.

## Three workflows / modes
**Interactive:** \`nyra\`; browse/search, input form, run, view, copy/export/favorite.
**Direct CLI:** \`nyra net tcp 192.168.1.10 443 --output json\`.
**Workflow:** \`nyra workflow run web-unreachable --target https://example.com\`.
CLI/TUI/workflows call one tool executor; command spellings are planned contracts until implementation verifies them.

## Supported use cases
- Is port 443 accepting TCP connections?
- Why is my website inaccessible: DNS, TCP, TLS, HTTP?
- How many days until a TLS certificate expires; what are issuer/SAN?
- Is local disk full, CPU/memory pressured, a process/listening socket present?
- How do I check Linux logs or large files without memorizing syntax?
- Does an SSH/SFTP or database endpoint respond?
- Run a repeatable read-only check and generate a shareable troubleshooting report.

## Quality and non-functional requirements
- Fast startup: **target** under 1s on a named baseline machine, not yet measured.
- Search latency: **target** under 100ms for built-in catalog, not yet measured.
- No UI blocking while async tasks run, with cancel and timeout.
- Bounded memory use for large logs; stream, never read unbounded files into RAM.
- Screen sizes: wide 3-panel, compact 2-panel, minimal/80x24 fallback.
- Least privilege, no shell injection, consent for non-read-only operations, authenticated remote connections.
- Actionable errors and explicit dependency/capability reporting.
- Document cross-platform differences; no unverified universal binary claims.
- Settings persist across restarts and portable moves, subject to media permissions.

## MVP vs v1
**First usable slice:** CLI, shared registry, TCP + diagnostic tools, search/TUI, 4 palettes, basic settings, one Web Diagnostics workflow, Windows/Linux portable tests. This is a milestone, not permission to skip critical safety gates.

**v1 aspirational scope:** complete planned 10 phases including knowledge, reports, remote checks, rule-based incident flows, security hardening and detailed release documentation. Work may be staged; unfinished features must be labeled honestly.

## Out of scope / constraints
No mandatory AI, cloud backend, web UI or per-server agent. No guarantee that raw packet scanning, native remote Windows management, audio hardware, USB writes or old Windows support is zero-install. Do not copy unlicensed cyberpunk game/Capri artwork/audio.
