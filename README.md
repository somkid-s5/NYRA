# NYRA

**NYRA — Your System. Your Tools. Anywhere.**

> Planning repository for a portable, offline-first System Administrator toolkit built with Rust. Inspired by cyberpunk terminal apps (including Capri); not a copy or fork.

**Status:** Specification / pre-implementation. No compiled application or functional commands exist yet.

## Product in one paragraph

NYRA is a portable IT operations companion used from a USB drive or downloaded as a platform-specific release. It combines a fast interactive terminal UI (TUI), a script-friendly direct CLI, diagnostic workflows, and an offline command knowledge base. Users search *what they want to inspect* (including Thai/English aliases) without needing to remember a command. Built-in Rust diagnostics are preferred; optional external or remote adapters are capability-gated. No AI account or network backend is required for core use.

## The three modes

- **Interactive:** `nyra` — browse/search tools, edit validated inputs, run and inspect results in a Ratatui UI.
- **Direct CLI:** `nyra net tcp <host> <port>` — predictable output, exit codes and JSON for automation.
- **Workflow:** `nyra workflow run web-unreachable --target https://example.com` — reusable multi-step diagnostics.

All commands above are *planned interfaces*, not currently working implementations. The earlier working title was **PocketOps**; **NYRA** is the final chosen project name. Executable name: `nyra` / `nyra.exe`.

## Read this first

**อ่านภาษาไทย:** [สรุปข้อตกลง NYRA ทั้งโปรเจกต์](docs/THAI_SUMMARY.md)

1. [Product requirements](docs/product-requirements.md)
2. [Decision log](docs/decisions.md) — what has already been agreed
3. [Master roadmap and backlog](docs/roadmap.md)
4. [Architecture and contracts](docs/architecture.md)
5. [Interactive TUI and UX](docs/tui-ux.md)
6. [Tool catalog and discovery](docs/tool-catalog.md)
7. [Workflow and incident diagnostics](docs/workflows.md)
8. [Themes, animation and sound](docs/experience.md)
9. [Portability and security](docs/portability-security.md)
10. [Verification and release criteria](docs/testing-release.md)
11. [AI agent skills](docs/agent-skills.md)
12. [Design references + four editable TUI mockups](docs/design-references.md)
13. [Codex Cloud handoff](docs/CODEX_START_HERE.md)
14. [First Codex task: NYRA-001](docs/CODEX_TASK_001.md)
15. [Agent working agreement](AGENTS.md)
16. [Example configuration](examples/config.toml)
17. [Example workflow definition](examples/web-unreachable.toml)

## Baseline

- Rust stable + Clap + Ratatui/Crossterm + Tokio; Serde/TOML for config; native Rust tooling first.
- Four configurable cyberpunk themes: Cyber Cyan, Neo Tokyo, Acid Grid, Crimson Noir.
- Optional sound/animation with graceful fallback.
- Read-only by default; never silently change production systems or save plaintext credentials.
- Per-platform artifacts, not one binary for every OS.
- Windows and Linux first; macOS/ARM64 after platform validation, Solaris via SSH; older Windows is not promised.
- Separate roadmap stages from **implemented/tested** status. No feature is complete merely because it appears in a spec.

## Theme previews

[Cyber Cyan](assets/mockups/nyra-cyber-cyan.svg) · [Neo Tokyo](assets/mockups/nyra-neo-tokyo.svg) · [Acid Grid](assets/mockups/nyra-acid-grid.svg) · [Crimson Noir](assets/mockups/nyra-crimson-noir.svg)

These are editable SVG **concept mockups**, not real screenshots of functioning software. Exact original PNG mockups from earlier chat are not yet committed.

## Initial instruction for Codex Cloud

Open [docs/CODEX_START_HERE.md](docs/CODEX_START_HERE.md), then read [AGENTS.md](AGENTS.md). Start with **Phase 0 validation and Phase 1 Rust skeleton**, preserving the contracts and creating tests before claiming a feature works.

## Credits & boundaries

Capri is an aesthetic reference only. Do not copy proprietary artwork, sound effects, branding, or code without checking licenses. All referenced skills/libraries require independent verification and security review before installation.

**Repository:** https://github.com/somkid-s5/NYRA
