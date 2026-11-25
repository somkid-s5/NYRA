# Decisions and History — NYRA

This log records the *decisions from the owner conversation* so agents do not repeatedly re-ask or redesign them. Recommendations are explicitly distinguished from commitments.

## Accepted / locked decisions
1. **Project identity:** NYRA (replacing temporary title PocketOps); short character-like name and terminal executable \`nyra\`.
2. **Purpose:** personal, portable toolkit for System Administrators. Avoid needing to remember hundreds of commands or install each diagnostic tool repeatedly.
3. **Primary experience:** interactive, keyboard-driven terminal UI (TUI) with search, categories, forms, results, favorites/history; no separate web service needed.
4. **Additional execution modes:** direct command-line use and reusable multi-step workflows; all share the same execution engine.
5. **Language:** Rust; aim for fast, low-overhead binaries.
6. **Distribution:** GitHub repository + downloadable platform-specific releases + USB flash drive operation; keep machine changes minimal.
7. **Offline-first:** core features work without a cloud account, internet, local AI model or API subscription.
8. **Task-based discovery:** search by goal/problem (curated English aliases), not only command name. Fuzzy/keyword search first, AI optional later.
9. **Native-first:** write common operations in Rust; integrate system tools or external programs only where appropriate and indicate missing dependencies.
10. **Aesthetics:** inspired by Capri/cyberpunk TUI style with energetic startup/feedback effects and sound, but actual implementation must fit real terminal constraints.
11. **Theme decision:** DO NOT pick one palette. Ship all four built-ins — Cyber Cyan (cyan/magenta), Neo Tokyo (purple/pink/orange), Acid Grid (lime/cyan/yellow), Crimson Noir (crimson/violet/black) — plus configuration/custom colors.
12. **Sound:** optional, configurable volume/events, ability to disable; animations also configurable.
13. **Safety:** diagnostics/read-only as default; sensitive/remote/admin actions guarded; no plaintext secrets.
14. **Development:** detailed phase plan from specification to v1.0, not a one-shot app generation.
15. **Agents:** Codex is primary developer in cloud; Gemini can review independently. Use project AGENTS.md, curated skills and explicit test gates.
16. **Documentation destination:** this GitHub repository holds project/source documentation intended for Codex Cloud handoff.

## Proposed implementation details (can change by ADR after testing)
- Rust crates: Clap, Ratatui, Crossterm, Tokio, Serde/TOML, Nucleo, Reqwest/Rustls, Sysinfo and selected DNS libraries.
- Start with a modular monolith / one Cargo package; move to workspace only if justified.
- Portable flag file controls local workspace directory and uses external folders for mutable data.
- F9 opens Appearance/Settings; Ctrl+K opens global tool search, F1 help, F2 workflows, F5 Favorites/Recent, and F6 Command Knowledge. Ctrl+F toggles the selected tool or knowledge-card favorite on the relevant screen. In Saved Tools, Ctrl+R toggles recent tracking; turning it off clears existing recent IDs and persists `[discovery].record_recent = false`, while turning it on starts a fresh list. The current Recent list persists at most 10 tool IDs and stores no targets or input values. Test platform key behavior.
- Wide/compact/minimal responsive TUI with target 80x24 fallback.
- SSH for agentless Linux/Solaris checks; WinRM/Windows remote is later and transport/library selection is not finalized.
- Rodio/platform audio backend is a candidate, not a guaranteed zero-dependency cross-OS feature.
- \`dist\`/GitHub Actions are candidates for packaging, subject to verification.
- Optional community tool packs, MCP server, optional AI diagnostics are post-v1 ideas.

## Explicit non-goals for initial implementation
- An AI chatbot or mandatory LLM integration.
- Installing a daemon/agent on each server.
- Built-in offensive scanning or exploit tooling.
- Universal one-file binary that runs unchanged on Windows, Linux, macOS and Solaris.
- Copying Capri proprietary assets or architecture without verification.
- Hundreds of tools, marketplace, cloud backend, accounts or subscriptions in the MVP.

## Evolution/history
- User described an always-available toolkit (Git/USB) after repeatedly lacking nmap or forgetting diagnostic commands.
- Initial **PocketOps** concept specified interactive/direct/workflow modes and native-first Rust diagnostics.
- User requested realistic Capri-inspired TUI image previews with startup/selection sounds.
- Four cyberpunk appearance variants were explored; user explicitly chose **all palettes + customizable settings**, not a favorite.
- A full 10-phase roadmap and a targeted AI-agent skill stack were proposed.
- User asked for a character-like project name; **NYRA** became the repository/project name.
- User explicitly requested preservation of prior agreements here for further development in Codex Cloud.

## 2026-11-17 — First usable version

The owner approved the [Core 12 diagnostics](tool-catalog.md#core-12-initial-build) in both the CLI and the guided TUI, with English search in the TUI. The first usable version also includes [Web Diagnostics](workflows.md) as a single URL check that reports DNS, TCP, TLS when applicable, and HTTP outcomes, including skipped steps. These checks cover both web connectivity and local system troubleshooting. Test the Windows USB and Linux SSH usage scenarios. The first executable TCP check remains a development checkpoint.

Use English for the CLI, TUI, and search aliases at the owner's request. The first usable version must demonstrate finding the TCP check using an English task query, checking disk space, and identifying a failed step in Web Diagnostics. Verify equivalent findings from the CLI and TUI, launch from a USB drive on Windows without Rust installed, and exercise the Linux TUI over SSH at 80×24.

The owner's latest language decision applies to the whole product: CLI, TUI, help, examples, and search aliases are English-only. Remove the earlier Thai alias examples; Thai-language project notes do not change the product language.

The CLI uses exit code 1 for a completed check that found a problem, including a TCP connection attempt that timed out. Use exit code 4 when the run reaches its deadline before it can report a finding. The JSON result carries execution status and the diagnostic finding separately.

## 2026-11-17 — End-to-end design

Preserve the chosen destination across Web Diagnostics steps and keep portable writes inside the workspace. The owner delegated these design choices. See [ADR 0001](adr/0001-preserve-workflow-endpoint.md) and [ADR 0002](adr/0002-isolate-portable-workspace.md) for the trade-offs and the [end-to-end design](end-to-end-design.md) for the delivery sequence.

## 2026-11-17 — Core diagnostic implementation notes

These are implementation details for the approved Core 12, not changes to the product requirements:

- CLI and TUI call one shared executor with typed inputs and bounded output.
- DNS uses the operating system's resolver configuration. The first version supports A, AAAA, CNAME, MX, TXT, and PTR lookups.
- HTTP makes one request, follows no redirects, reads no response body, returns an allowlist of headers, and omits URL query and fragment values from results.
- TLS inspection performs an inspection-only handshake. It records the result from the normal system certificate verifier, continues only long enough to read the peer certificate, and sends no application data. This allows expired or untrusted certificates to be examined without treating them as trusted.
- Ping passes validated arguments directly to the operating system's `ping` utility; it does not interpolate input into a shell command. The utility is optional, bounded by count and deadline, and ICMP filtering can make a host appear unreachable.
- Local interface output contains addresses and netmasks only. Gateway and DNS-server discovery are not included in this implementation.
- Windows and Linux are declared as implementation targets in registry metadata; smoke-test status remains explicit for every tool and must not be treated as platform verification.

## 2026-11-19 — Initial report export implementation

The first report file export is limited to Web Diagnostics in the CLI and requires `--report-json <PATH>` on the run command. It writes a separate allowlisted JSON schema, always redacts the full target, omits observer identity and raw result/step data, is capped at 64 KiB, and refuses to overwrite an existing destination. The existing `--output json` stdout result remains a separate, raw `RunResult` contract. HTML, CSV, generic tool reports, and TUI export remain open.

## 2026-11-23 — Typed workflow definitions and report formats

NYRA defines a serializable typed workflow schema and validator. Conditions are a bounded AST over prior step status/finding, and parameters bind only from declared inputs, literals, or typed outputs from earlier steps. The CLI can validate custom TOML definitions with `nyra workflow validate <PATH>` and run them sequentially with `nyra workflow run-file <PATH>` through the shared executor. Trusted-registry input types include `string`, `dns_hostname`, `hostname`, `ip_address`, `url`, `file_path`, `record_types`, `port`, `timeout_seconds`, `count`, `integer`, and `boolean`; semantic checks reject invalid values before execution. Only registered read-only tools are accepted; the version-1 workflow result contains safe step summaries and does not include workflow input values or raw tool output. Automated coverage includes typed schema validation, prior-step condition execution, shared-executor file workflows, and input/path redaction. A TUI workflow builder, parallel/DAG execution, and report export for arbitrary custom workflow results remain open.

The Web Diagnostics CLI additionally supports one explicit `--report-html <PATH>` or `--report-csv <PATH>` destination per run, alongside JSON. These formats use sanitized projections, redact the target/observer, omit raw tool data, cap output at 64 KiB, and refuse overwrite. The Rust report API can serialize generic tool results, but generic CLI report flags and TUI export remain open.

## Decision process

Record future changes here as: date; topic; current behavior; decision; why; alternatives; implementation implications; who approved it. Do not silently overwrite locked decisions.
