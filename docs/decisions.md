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
8. **Task-based discovery:** search by goal/problem (Thai/English aliases), not only command name. Fuzzy/keyword search first, AI optional later.
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
- F9 opens Appearance/Settings; Ctrl+K search, F1 help, F2 workflows, F5 favorites. Test platform key behavior.
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

## Decision process
Record future changes here as: date; topic; current behavior; decision; why; alternatives; implementation implications; who approved it. Do not silently overwrite locked decisions.
