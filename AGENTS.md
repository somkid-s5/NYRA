# NYRA — Repository Instructions for AI Coding Agents

Read this file before writing code. Then read docs/CODEX_START_HERE.md, docs/decisions.md, docs/product-requirements.md, docs/architecture.md, docs/testing-release.md and the relevant feature document. The repository README links the full specifications.

## Source of truth and current status
- Product is **NYRA** (formerly PocketOps). Executable is **nyra** (nyra.exe on Windows). Do not rebrand without explicit approval.
- Current status: **Phase 1 development; 0/10 release phase gates closed**. Custom typed workflow TOML files can be validated and run sequentially from the CLI through the shared tool executor. On 2026-11-23, Kali WSL passed `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, and `cargo test --locked --all-targets` (124 unit, 33 CLI). Windows x64 GNU format, check, and Clippy pass; its full test command remains blocked at link time by AWS-LC/MinGW's undefined `nanosleep64` symbol from `pthread_time.h`. CI, clean-host Windows/Linux, TUI PTY, platform smoke, and release gates remain open. Check the roadmap before claiming platform support, artifacts, or release readiness.
- Git repository is authoritative for code and specifications. Do not invent prior implementation, measurements or supported OS combinations.
- Core product goal: fast, discoverable, USB-portable, offline-first System Administrator diagnostics, with CLI + interactive TUI + workflows, not an AI-first product.
- Distinguish **agreed requirements**, **recommended implementation details**, and **future ideas**; record any changes in docs/decisions.md.
- Use working code and automated tests to prove completion. Keep documentation aligned with reality.

## Starting implementation
1. Check the actual repository tree and current Rust dependencies. Avoid overwriting user contributions.
2. Propose/execute smallest verified step from docs/roadmap.md. Start at Phase 0 validation / Phase 1; do not attempt v1.0 in one giant generation.
3. Write tests for requirements, implement, then run format/lint/tests. Report executed commands with pass/fail or explicitly say what could not run.
4. Prefer a single Rust Cargo package and a modular monolith until scaling justifies a workspace.
5. Implement reusable Tool Registry, typed inputs, result model and executor before multiplying tool-specific UX.
6. CLI and TUI **must share the same tool implementations**; never fork network logic by UI.
7. Use Clap for CLI and Ratatui/Crossterm for TUI, Tokio for async, Serde/TOML for data. Treat dependency names as candidates and verify current compatibility before pinning.
8. Design app states and keyboard navigation first; use Ratatui TestBackend and PTY testing where practical.
9. Keep source modular, maintain small PRs, and avoid adding dependencies without demonstrable benefit.

## Security boundaries
- Default tools are read-only and use least privilege. Avoid permanent changes, service restarts, deletions, invasive network scans, or bulk host probing in early releases.
- Only run against authorized systems. Validate target host, port, file path and timeout; cap concurrency and resource use.
- Never pass untrusted values to a shell interpolated string; prefer typed argument arrays/native implementations.
- Do not store plaintext secrets; redact credentials, tokens, internal targets and sensitive logs from stored history, reports and CLI debug output.
- Respect OS/endpoint protection policies; never attempt bypasses.
- Remote SSH must verify host keys and use agents/keys/secure credential sources. Windows remote transport likewise must authenticate and validate certificates.
- Optional external tools must be discovered and reported as unavailable if dependencies are missing; never claim universal zero-install support.
- Third-party AI skills and assets are untrusted until their content, sources and licenses are reviewed.

## UX and accessibility
- Support English discovery via curated aliases, not unsupported claims of AI comprehension.
- Keyboard-first, discoverable, fast; work in 80x24 fallback as well as wide terminals. No key binding may unpredictably conflict with text entry.
- Four configurable themes: Cyber Cyan, Neo Tokyo, Acid Grid, Crimson Noir. Themes are data, not hardcoded branches.
- Optional animations and sound MUST be interruptible/disableable; CLI and headless/SSH paths must default quiet.
- Avoid copying Capri's assets/code/music without license checks; it is an inspiration only.
- Follow test and accessibility requirements in docs/tui-ux.md and docs/experience.md.

## Testing / gates
- Before merging, run at least cargo fmt --check, cargo clippy --all-targets -- -D warnings, cargo test (when project exists), and applicable integration/PTY/snapshot tests.
- Add OS-specific tests for Windows and Linux. Do not call a platform supported without clean-environment smoke testing.
- Define stdout/stderr, exit codes and JSON schemas for automation. A valid diagnostic finding is not the same as an application execution error.
- No v1.0 claim without security, UX, portability and release criteria in docs/testing-release.md.

## Diagrams / docs
- For new or updated Markdown documenting infrastructure, architecture, topology, system design, data flow: use the owner's **Diagram Design Skill** as the primary diagram tool when available. Produce professional image diagrams, save to assets/diagrams/, reference via relative paths, and verify rendering/assets. Preserve editable source when supported. Never substitute rough ASCII art or throwaway Mermaid.
- **Important:** The required Diagram Design Skill was not available to the authoring session when this baseline documentation was created. Therefore no diagram has been fabricated or represented as skill-produced. Arrange skill availability before drawing architecture diagrams.
- Be honest about unverified skill recommendations and build/test status.

## AI handoff
- Primary implementer: Codex Cloud; independent reviewer: Gemini or another reviewer if available.
- Reviewer must inspect code, test output, diffs and user-facing flows independently, not accept developer self-report.
- If requirements conflict, follow the decision log, explain the conflict, and make the smallest reversible choice. Record ADR for meaningful architecture changes.
- Keep README status and docs/roadmap.md checkboxes accurate after each phase.
