# NYRA — Master Roadmap & Implementation Backlog

**State: 0/10 phases verified.** Planned versions are roadmap targets; not releases. Phase gates refer to real tests, not a status report.

| Phase | Planned version | Focus | Exit gate |
|---|---|---|---|
| 0 | Planning | PRD, UI, contracts, platform/security spec | All requirements testable; gaps recorded |
| 1 | v0.1 foundation | Rust CLI, tool registry, executor, result/error, CI | One executable tool via CLI + tests |
| 2 | v0.1 tools | 12 foundational diagnostics, platform adapters | All tests incl. invalid/timeout cases |
| 3 | v0.2 | Ratatui TUI, search, forms, progress, keyboard | End-to-end use without memorized commands |
| 4 | v0.3 | 4 themes, custom colors, effects, optional audio | Live switching, persistence, graceful fallback |
| 5 | v0.4 | Knowledge cards, Thai/English alias discovery | Queries find correct tools; safe copy/run |
| 6 | v0.5 | Workflow engine, builder basics, reports | Web flow runs, cancels, exports |
| 7 | v0.6 | Windows/Linux/ARM64 packaging, USB portability | Clean-VM and removable/read-only media tests |
| 8 | v0.8 | SSH/Solaris/Windows, Oracle/MSSQL, optional tools | Authenticated and read-only remote tests |
| 9 | v1.0 | Incident mode, hardening, performance, docs | Security/accessibility/UX/release acceptance |

## Phase 0 detailed checklist
- [ ] Confirm CLI namespace and flags, error/exit policy and JSON versioning.
- [ ] Confirm Tool Registry schema, input types, outputs and platform capability model.
- [ ] Approve TUI user journeys, focus/key bindings, narrow-screen fallback.
- [ ] Define color tokens/custom theme format and audio fallback.
- [ ] Select Windows/Linux target triples and clean-build test matrix.
- [ ] Review security boundaries/permissions/credentials/history redaction.
- [ ] Prepare professional architecture/data-flow diagram via Diagram Design Skill **when available**; currently not delivered.
- [ ] Split specs into small tickets and unit/integration/smoke test criteria.

## Phase 1 detailed checklist
- [ ] Cargo app named nyra with Rust stable and locked deps.
- [ ] Clap commands and \`--help\`; no TUI required for command invocation.
- [ ] Tool metadata model and discovery registry.
- [ ] Validated typed inputs, execution context, timeout and cancellation.
- [ ] Structured execution status/findings/error, text and JSON formatting.
- [ ] Native TCP connect end-to-end slice with deterministic tests.
- [ ] Minimal tracing and sensitive-data redaction.
- [ ] CI format, lint, test; clear README build instructions.

## Phase 2 detailed checklist
- [ ] TCP Port Check, DNS Lookup, Reverse DNS, HTTP Check, TLS Certificate.
- [ ] Local Network Info, ICMP Ping adapter/capability handling.
- [ ] OS Info, CPU, Memory, Disk, SHA-256.
- [ ] Each tool has help, inputs, platform support, examples and test cases.
- [ ] Non-available or privileged operations fail clearly without crashes.

## Phase 3 detailed checklist
- [ ] TUI state machine; header/search/categories/tool list/form/results/footer.
- [ ] Keyboard navigation, focus management, Esc/Cancel, clean terminal restore.
- [ ] Responsive wide, compact and 80x24 fallback, UTF-8/Thai handling.
- [ ] Input validation, progress, interactive cancel and failure explanations.
- [ ] Favorites and recent tools basic UI plus privacy settings.
- [ ] TestBackend/PTY tests and real user task-based walkthroughs.

## Phase 4 detailed checklist
- [ ] Palette tokens separated from widgets with the four agreed themes.
- [ ] External custom-theme TOML validation; runtime switch + config save.
- [ ] Optional sounds by event; volume/mute and headless no-audio behavior.
- [ ] Non-blocking startup/selection/progress effects; reduced motion/no color.

## Phase 5 detailed checklist
- [ ] Fuzzy + aliases + deterministic ranking (Thai and English).
- [ ] Offline command knowledge: examples, caution, OS, copy, favorites.
- [ ] Input templates for command recipes; confirmation of risky actions.
- [ ] Documentation of search limitations (not general natural-language AI).

## Phase 6 detailed checklist
- [ ] Typed workflow definition, variables, ordered conditional steps.
- [ ] Bounded concurrency, retries (if safe), timeout, cancellation, progress.
- [ ] DNS → TCP → TLS → HTTP initial Web Diagnostics template.
- [ ] HTML and JSON reports first; CSV for tabular data.
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
| ID | Size | Scope | Dependencies |
|---|---|---|---|
| NYRA-001 | S | Cargo/Clap skeleton + CI | Phase 0 |
| NYRA-002 | M | Tool registry + input/result contracts | 001 |
| NYRA-003 | S | Native TCP checker + tests | 002 |
| NYRA-004 | M | Basic TUI navigation and layouts | 002 |
| NYRA-005 | L | Remaining 11 core diagnostics | 002 |
| NYRA-006 | M | Config, theme tokens and 4 palettes | 004 |
| NYRA-007 | M | Search/aliases/favorites/knowledge | 004 |
| NYRA-008 | L | Workflows + reports | 003,005 |
| NYRA-009 | M | Portable packaging + clean OS smoke | 003,004 |
| NYRA-010 | L | Remote/incident and release hardening | earlier gates |

## Delivery rules
- Work in small, reviewable steps. Do not build every module at once.
- Keep docs accurate, update this checklist only with evidence of tests.
- Record deviations in docs/decisions.md and explain tradeoffs.
- Build and smoke-test portable behavior throughout; do not delay *thinking about portability* until Phase 7.
