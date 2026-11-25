# NYRA — Quality, Test Strategy and Release Gate

## Principle
Only code that has run on an appropriate environment and passed tests is implemented.

**Latest local verification (2026-11-23):** Kali Linux under WSL passed `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, and `cargo test --locked --all-targets` (124 unit tests and 33 CLI tests). Windows x64 GNU `fmt --check`, `check --locked --all-targets`, and Clippy with `-D warnings` pass. Windows `cargo test --locked --all-targets` remains blocked during linking by AWS-LC/MinGW's undefined `nanosleep64` symbol from `pthread_time.h`. Kali WSL is runtime evidence for that environment, not a clean Ubuntu host. Hosted CI and clean Windows/Ubuntu verification remain pending.

**Workflow coverage:** Tests cover typed workflow schema validation, read-only tool and scalar-output allowlists, typed inputs and bindings, conditions over earlier step statuses/findings, and sequential file execution through the shared executor. CLI coverage verifies custom-file validation, schema-version-1 results, the local `system.info`/conditional `system.cpu` example, typed file input through `file.sha256`, and that path values and rejected credential-bearing URLs are not echoed in results or errors.

Other coverage includes Web Diagnostics DNS/TCP/TLS/HTTP ordering, cancellation and report schema/redaction/no-overwrite behavior; settings and theme persistence, custom-theme validation/import/export and no-color behavior; TUI navigation/search/copy/favorites and 80×24 rendering; motion settings and interruptible startup; and offline knowledge-card lookup/search. The Solaris cards were checked against Oracle references but have not been executed on Solaris. FreeBSD cards cite official manuals but have not been run on FreeBSD.

The machine's default Windows MSVC toolchain lacks `link.exe` and Windows SDK libraries. Local checks therefore used Rust `stable-x86_64-pc-windows-gnu` with MSYS2 MinGW on PATH and `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` set to MinGW's `gcc.exe`. A separate MSVC build has not been verified.

## Test pyramid
| Level | Required checks |
|---|---|
| Unit | validators, ports, URL parsing, tool registry, result/error semantics, search ranking, config/theme validation |
| Integration | CLI runs native diagnostic; JSON schema and exit codes; executor reuse by workflows |
| TUI render | Ratatui TestBackend layout, focus states, search results, resizing, 80x24 fallback |
| Terminal / PTY | key events, Ctrl+C, clean teardown, no terminal corruption, Windows/Linux input |
| Cross-platform | clean build/run on target Windows/Linux versions, no Rust installed |
| Security | command injection, path validation, secret redaction, remote auth/host key, dependency handling |
| Workflow | ordered steps, skip semantics, partial failures, cancel, timeout, report correctness |
| Experience | theme switching/persistence, reduced motion, audio-disabled/headless fallback |
| Human UX | task-based discovery, duplicate navigation, response comprehension, time to complete check |

## Suggested development commands (once Rust exists)
~~~sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
~~~
Only claim these passed if they actually ran and passed. Account for feature/platform-specific dependencies when using --all-features.

## Critical acceptance scenarios
- [ ] Run from USB / extracted archive on clean Windows host with no Rust/Cargo.
- [ ] Run on clean supported Ubuntu and RHEL with no Rust/Cargo.
- [ ] No changes to user/system config in portable mode, except opted-in workspace writes.
- [ ] Search English task queries and show useful top result.
- [ ] Walk through F5 Favorites/Recent, Ctrl+F, and Ctrl+R on a real terminal; verify disabling clears recent IDs, re-enabling starts fresh, the ten-ID cap, and that targets/input values are absent from saved preferences.
- [ ] TCP check without Nmap, handling open/refused/timeout correctly.
- [ ] TLS and HTTP results distinguish protocol error vs invalid input.
- [ ] CLI and TUI produce semantically same findings.
- [ ] JSON output machine-readable, no ANSI/no sound/progress pollution.
- [ ] Switch all themes without restart, persist config and recover from bad custom config.
- [ ] Audio backend absent/headless: no crash and no blocking.
- [ ] Cancel running tasks; Ctrl+C restores terminal.
- [ ] At 80x24 keyboard can reach search, tool form, result, settings and quit.
- [x] CLI writes sanitized Web Diagnostics JSON, HTML, and CSV reports with explicit paths, target redaction, raw-data omission, no-overwrite behavior, and invalid-input handling.
- [ ] Complete broader report acceptance: generic tool CLI reports, TUI export, and clean-host walkthrough.
- [ ] Missing optional external tool produces actionable Requires Setup response.
- [ ] No plaintext credentials leaked to history, logs or reports.
- [ ] Release archives include checksums, version, license and release notes after license chosen.
- [x] Validate and run a typed custom workflow TOML file through the shared executor; automated coverage includes typed schema/condition behavior and input/path redaction.

## CLI status/contract tests
Differentiate success of execution from diagnostic finding (e.g. refused port). Document stable JSON schema and exit codes after Phase 0 approval. Validate stdout/stderr separation, timeout, cancellation and nonzero error behavior.

Optional CLI tracing uses `--log-level error|warn|info|debug` and is disabled by default. It writes only to stderr and records allowlisted metadata; targets, paths, URL details, and raw error messages are excluded. The CLI integration suite checks that debug logs leave JSON stdout parseable and do not contain the URL host, path, or query secret.

## Performance budget (provisional targets, not measured)
- Startup time on identified baseline host: <1 second.
- Local discovery for built-in catalog: <100ms interactive response.
- Async network checks do not block TUI redraw.
- No unbounded log buffering; memory under an empirically established budget.
- Short animation frames and no startup delay in --quiet/noninteractive runs.

Measure and publish baseline hardware and actual figures before making release claims.

## CI and packaging
On PR: format/lint/test, vulnerability/dependency audit where available, platform builds. On release tag: build separate target artifacts, run smoke tests, generate SHA256 and notes, publish GitHub Releases. Tools such as cargo-dist may be chosen only after verifying current target support. Bundle only licensed assets and dependencies.

## Release stages
- v0.1: CLI + foundation/native tool set
- v0.2: interactive TUI
- v0.3: themes/sound
- v0.4: discovery/knowledge
- v0.5: workflows/reports
- v0.6: portable release pipeline
- v0.8: remote/advanced tooling
- v1.0: incident mode + stability, security, performance, docs

## Definition of Done for a feature
Code + tests + documentation + error states + platform notes + privacy assessment + successful CI + reviewer verification. If only a mockup or proposal exists, mark feature 'planned', not 'done'.
