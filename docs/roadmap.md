# NYRA — Master Roadmap & Implementation Backlog

**State: Phase 1 in progress, 0/10 phase gates closed.** The worktree includes the shared registry and executor, 12 diagnostics, guided TUI, appearance and discovery features, 19 offline knowledge cards, Web Diagnostics, sanitized Web Diagnostics JSON/HTML/CSV reports, and typed custom workflow TOML support. Custom definitions can be validated and run sequentially with `workflow validate` and `workflow run-file` through the shared tool executor; the bundled local system health example validates and runs. The TUI workflow builder, generic custom-workflow report export, and TUI report export remain open.

On 2026-11-23, Kali WSL passed `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, and `cargo test --locked --all-targets` (124 unit, 33 CLI). Windows x64 GNU format, check, and Clippy pass; the full test suite remains blocked at link time by AWS-LC/MinGW's undefined `nanosleep64` symbol from `pthread_time.h`. Hosted CI, clean-host Windows/Linux, TUI PTY, task-based human walkthroughs, platform smoke tests, and release gates remain open. Do not claim platform support or release readiness from these local checks. Planned versions are roadmap targets.

| Phase | Planned version | Focus | Exit gate |
|---|---|---|---|
| 0 | Planning | PRD, UI, contracts, platform/security spec | All requirements testable; gaps recorded |
| 1 | v0.1 foundation | Rust CLI, tool registry, executor, result/error, CI | One executable tool via CLI + tests |
| 2 | v0.1 tools | 12 foundational diagnostics, platform adapters | All tests incl. invalid/timeout cases |
| 3 | v0.2 | Ratatui TUI, search, forms, progress, keyboard | End-to-end use without memorized commands |
| 4 | v0.3 | 4 themes, custom colors, effects, optional audio | Live switching, persistence, graceful fallback |
| 5 | v0.4 | Knowledge cards, English alias discovery | Queries find correct tools; safe copy/run |
| 6 | v0.5 | Workflow engine, builder basics, reports | Web flow runs, cancels, exports |
| 7 | v0.6 | Windows/Linux/ARM64 packaging, USB portability | Clean-VM and removable/read-only media tests |
| 8 | v0.8 | SSH/Solaris/Windows, Oracle/MSSQL, optional tools | Authenticated and read-only remote tests |
| 9 | v1.0 | Incident mode, hardening, performance, docs | Security/accessibility/UX/release acceptance |

## Phase 0 detailed checklist
- [ ] Confirm CLI namespace and flags, error/exit policy and JSON versioning.
- [ ] Confirm Tool Registry schema, input types, outputs and platform capability model.
- [ ] Approve TUI user journeys, focus/key bindings, narrow-screen fallback.
- [x] Define semantic color tokens, versioned custom theme format, and invalid-theme fallback.
- [ ] Define audio fallback.
- [ ] Select Windows/Linux target triples and clean-build test matrix.
- [ ] Review security boundaries/permissions/credentials/history redaction.
- [ ] Complete architecture and data-flow diagrams. The [component overview](architecture.md#component-architecture) is available.
- [ ] Split specs into small tickets and unit/integration/smoke test criteria.

## Phase 1 detailed checklist
- [x] Cargo app named nyra with Rust stable and locked deps.
- [x] Clap commands and \`--help\`; no TUI required for command invocation.
- [x] Tool metadata model and discovery registry.
- [x] Validated typed inputs, timeouts, and cancellation for the TCP slice.
- [x] Structured execution status, findings, and errors with text and JSON formatting for the TCP slice.
- [x] Native TCP connect end-to-end slice with deterministic tests.
- [x] Minimal opt-in tracing on CLI stderr; allowlisted fields exclude targets and raw input values (Windows GNU CLI integration test).
- [x] Windows/Ubuntu CI format, lint, and test workflow configured; README build instructions present. Windows GNU and Kali WSL checks pass; hosted CI and clean Windows/Ubuntu execution remain open.

## Phase 2 detailed checklist
- [ ] TCP Port Check, DNS Lookup, Reverse DNS, HTTP Check, TLS Certificate.
- [ ] Local Network Info, ICMP Ping adapter/capability handling.
- [ ] OS Info, CPU, Memory, Disk, SHA-256.
- [ ] Each tool has help, inputs, platform support, examples and test cases.
- [ ] Non-available or privileged operations fail clearly without crashes.

## Phase 3 detailed checklist
- [ ] TUI state machine; header/search/categories/tool list/form/results/footer.
- [ ] Keyboard navigation, focus management, Esc/Cancel, clean terminal restore.
- [ ] Responsive wide, compact and 80x24 fallback, Unicode handling.
- [ ] Input validation, progress, interactive cancel and failure explanations.
- [x] Basic Favorites/Recent UI and privacy behavior: F5 opens saved tools, Tab switches lists, Ctrl+F toggles a tool favorite, and Recent stores at most 10 tool IDs without targets or input values. Ctrl+R disables tracking, clears the list and persists the opt-out; enabling tracking starts fresh. PTY and task-based human walkthroughs remain open below.
- [ ] TestBackend/PTY tests and real user task-based walkthroughs.

## Phase 4 detailed checklist
- [x] Four built-in palette data entries and Ctrl+T live switching.
- [x] Built-in theme selection loads/saves through TOML config; portable-path precedence, config preservation, and failure fallback have local Windows GNU and Kali WSL tests.
- [x] Versioned external custom-theme TOML parsing, semantic color/contrast validation, startup loading, graceful fallback, and Ctrl+T selection when a valid custom file is present.
- [x] F9 appearance screen with keyboard preview/apply and return to the prior screen.
- [x] Custom-theme CLI validate/import/export with bounded reads and explicit `--force` overwrite.
- [x] Respect `NO_COLOR` and `[appearance].no_color` in every TUI screen and the theme preview.
- [ ] Optional sounds by event; volume/mute and headless no-audio behavior.
- [x] Bounded indeterminate Running marker with a normal 150 ms cadence and bounded 75/300 ms fast/slow options; controlled by `animations` and `reduced_motion`, with static SSH defaults.
- [x] F9 animation/reduced-motion controls and bounded slow/normal/fast speed selection; changes persist asynchronously and apply to the current session.
- [x] Three-frame header startup spinner follows motion settings, skips on any key, and ends within 900 ms.
- [ ] Selection effects and optional sound.

## Phase 5 detailed checklist
- [x] Fuzzy + aliases + deterministic ranking (English), including typo-tolerant ranking over curated knowledge metadata.
- [x] Nineteen offline read-only command cards for Windows, Linux, macOS, Solaris, and FreeBSD with examples, risk, side effects, privileges, related diagnostics, and deterministic multi-term CLI search; `nyra knowledge list/search/show` prints cards without execution.
- [x] TUI Command Knowledge on F6 with local search/discovery, static-command copy request, and persisted card favorites; Ctrl+K remains global tool search.
- [x] Add five Solaris read-only command cards for filesystem space, large-file discovery, configured host lookup, one authorized TCP connection attempt, and local TCP socket status.
- [x] Add FreeBSD read-only command references for filesystem/inode capacity and local listening TCP sockets.
- [ ] Extend command-card coverage to additional operating systems.
- [ ] Input templates for command recipes; confirmation of risky actions.
- [x] Document that offline knowledge search uses curated metadata and deterministic typo-tolerant ranking, not general natural-language AI.

## Phase 6 detailed checklist
- [x] Typed serializable workflow definition and validator for declared inputs, ordered parameter bindings, scalar outputs, and bounded status/finding conditions.
- [x] Validate and run custom TOML workflow files sequentially through the shared executor, with typed inputs, timeout/cancellation, and sanitized step summaries; this includes the bundled local system health example.
- [ ] Parallel workflows, retries (if safe), TUI workflow builder and editing UX.
- [ ] DNS → TCP → TLS → HTTP initial Web Diagnostics template (DNS/HTTP, failure/cancel, and trusted local HTTPS paths pass in local tests; trusted HTTPS is verified on Kali WSL only; clean-host matrix and remaining workflow/report features remain open).
- [x] Sanitized JSON, HTML, and CSV report export for the Web Diagnostics CLI through explicit `--report-json`, `--report-html`, or `--report-csv` paths; target redaction, raw-data omission, 64 KiB cap, and no-overwrite behavior.
- [ ] Generic tool report CLI options and TUI report export.
- [ ] Workflow builder phase 1: sequential + simple conditions.

## Phase 7 detailed checklist
- [ ] GitHub CI build/test matrix; release artifacts and checksums.
- [ ] USB portable flag and writable/read-only behavior.
- [ ] Build per OS+arch and clean virtual machine smoke tests.
- [ ] Platform compatibility table generated from test evidence.
- [ ] No unverified claims of Windows Server 2008 R2 or Solaris native support.

## Phase 8 detailed checklist
- [ ] Target profiles with secure credential references.
- [ ] SSH host-key checking, authorized remote Linux/Solaris diagnostics.
- [ ] Windows remote auth/TLS design and tested adapter.
- [ ] Oracle and SQL Server: distinguish port tests vs real DB health.
- [ ] External tool discovery and Nmap optional integration; license review.

## Phase 9 detailed checklist
- [ ] Incident symptom library: web unreachable, timeout, slow server, disk, database, cert.
- [ ] Evidence-based rule engine with limitations and next-step hints.
- [ ] Security + dependency audit, redaction tests, UX/PTY/snapshot tests.
- [ ] Release notes, installation/USB docs, license and contributor instructions.
- [ ] No outstanding critical bugs; real release smoke-tested.

## Prioritized implementation queue
| ID | Size | Scope | Dependencies | Worktree status |
|---|---|---|---|---|
| NYRA-001 | S | Cargo/Clap skeleton + CI | Phase 0 | Implemented; local Windows GNU checks pass; CI and clean-host gates open |
| NYRA-002 | M | Tool registry + input/result contracts | 001 | Implemented; unit and CLI tests pass on local Windows GNU |
| NYRA-003 | S | Native TCP checker + tests | 002 | Local Windows x64 GNU tests passed |
| NYRA-004 | M | Basic TUI navigation and layouts | 002 | Implemented; TestBackend tests pass; PTY gate open |
| NYRA-005 | L | Remaining 11 core diagnostics | 002 | Implemented; local Windows GNU tests pass; Linux and clean-host gates open |
| NYRA-006 | M | Config, theme tokens and 4 palettes | 004 | Partially implemented: built-in persistence, versioned custom TOML load/validation/cycling, F9 preview/apply, CLI import/export, and a bounded Running indicator with SSH-aware defaults, reduced-motion config, and an interruptible startup spinner; selection effects and audio remain open |
| NYRA-007 | M | Search/aliases/favorites/knowledge | 004 | Partially implemented: curated English search and typo-tolerant ranking, 19 offline Linux, Windows, macOS, Solaris, and FreeBSD cards (five Solaris and two FreeBSD references), CLI list/search/show, TUI knowledge-card discovery/copy/favorites, and tool Favorites/Recent with an IDs-only ten-item cap and Ctrl+R opt-out; additional OS coverage, PTY validation, and human walkthroughs remain open |
| NYRA-008 | L | Workflows + reports | 003,005 | Typed workflow schema/validator and custom TOML `validate`/sequential `run-file` via the shared executor are implemented; sanitized JSON/HTML/CSV report export is available for Web Diagnostics CLI runs. Generic custom-workflow report CLI options, TUI export, builder, and clean-host matrix remain open. On 2026-11-23, Kali WSL passed format, Clippy, and all-target tests (124 unit/33 CLI); Windows x64 GNU format/check/Clippy pass, while full tests remain blocked by AWS-LC/MinGW's undefined `nanosleep64` symbol from `pthread_time.h`. |
| NYRA-009 | M | Portable packaging + clean OS smoke | 003,004 | Planned |
| NYRA-010 | L | Remote/incident and release hardening | earlier gates | Planned |

## Delivery rules
- Work in small, reviewable steps. Do not build every module at once.
- Keep docs accurate, update this checklist only with evidence of tests.
- Record deviations in docs/decisions.md and explain tradeoffs.
- Build and smoke-test portable behavior throughout; do not delay *thinking about portability* until Phase 7.
