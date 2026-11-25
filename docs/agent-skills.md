# NYRA — Curated AI Agent Skills and Agent Workflow

## Purpose
Use a lean, high-signal set of skills so Codex Cloud can deliver high-quality Rust CLI/TUI code, meaningful testing and UX review. Gemini may serve as an independent reviewer. Skills augment instructions but do not override code/tests/security requirements in AGENTS.md.

**Current status (2026-11-17):** NYRA remains planning-only. The selected stack is installed for Codex and Gemini CLI at project scope (`.agents/skills/`) and user scope (`~/.agents/skills/`). Root `skills-lock.json` records external project skill sources, commits, and hashes. Installation used Skills CLI 1.9.0 with Node.js 24.22.2.

Installed stack:
- AI Hero / Matt Pocock flow: `ask-matt`, `to-spec`, `to-tickets`, `implement`, `implement-spec`, `tdd`, `code-review`.
- Rust and documentation: `rust-expert`, `rust-cli-clap`, `rust-tui-ratatui`, `technical-writing`.
- CLI and security: `agent-cli-design`, `code-security`, `semgrep`.
- Architecture diagrams: `diagram-design`.
- NYRA-specific: `nyra-tool-development`, `nyra-tui-ux-review`, `nyra-portability-audit`, `nyra-release-verification`.

`semgrep` is an instruction skill; it does not install the Semgrep scanner/CLI. The Semgrep skills license restricts distribution, so those project copies are kept local and excluded from Git. The upstream `rust-expert` folder has no per-skill license grant, so its project copy is also kept local and excluded from Git. Do not publish or redistribute either set of files. Other selected third-party sources declare MIT licenses. The installed files were targeted to the named agent directories; runtime loading was not separately tested.

## Candidate skills (review before install)
| Role | Candidate | Upstream |
|---|---|---|
| Main development flow/spec/TDD/review | AI Hero / Matt Pocock skills | https://github.com/mattpocock/skills |
| Rust systems engineering | rust-expert | https://github.com/BjornMelin/dev-skills |
| Clap CLI design | rust-cli-clap | https://github.com/BjornMelin/dev-skills |
| Ratatui TUI design/testing | rust-tui-ratatui | https://github.com/BjornMelin/dev-skills |
| CLI UX/automation ergonomics | agent-cli-design | https://github.com/ggonzalez94/skills |
| Security review | code-security | https://github.com/semgrep/skills |
| Technical writing | technical-writing | https://github.com/BjornMelin/dev-skills |
| Static analysis workflow | semgrep | https://github.com/semgrep/skills |
| Architecture diagram design | Diagram Design Skill | https://github.com/cathrynlavery/diagram-design |

Skill selection order: check existing skills in environment first; review their SKILL.md and all scripts/permissions; only install those available and needed. If a named repo/path no longer exists, choose an equivalent maintained skill after review and record the decision. Avoid duplicate master workflow systems.

## Installation and updates

The Skills CLI installs into the project by default; pass `-g` for the user-global scope. Select only the named skills needed from each repository. Project installs are recorded in `skills-lock.json`; check both scopes with `npx skills list` and `npx skills list -g`.

~~~powershell
cd C:\Users\somkidso\Desktop\01_Projects\NYRA

# Example project install for Codex and Gemini CLI.
npx skills add BjornMelin/dev-skills -a codex -a gemini-cli -s rust-expert -s rust-cli-clap -s rust-tui-ratatui -s technical-writing
npx skills add ggonzalez94/skills -a codex -a gemini-cli -s agent-cli-design
npx skills add semgrep/skills -a codex -a gemini-cli -s code-security
npx skills add cathrynlavery/diagram-design -a codex -a gemini-cli -s diagram-design

# Repeat a command with -g for user-global installation.
npx skills add ggonzalez94/skills -a codex -a gemini-cli -s agent-cli-design -g
npx skills list
npx skills list -g
~~~

The list and lockfile are the source of truth for installed names and revisions. A skill install does not install separate tools such as the Semgrep CLI.

## Preferred roles
- **Codex Cloud / Implementer:** reads AGENTS.md and Product Spec, chooses a small ticket, writes code/tests, runs and reports test results, makes reviewable changes.
- **Gemini / Reviewer:** checks design compliance and actual diff, probes UX, handles alternate solutions, tests quality/security; no rubber-stamping.
- **Verifier / Release Gate:** checks target OS compatibility, CLI behavior, saved artifacts, changelog and traceability. May be same person/tool only if independently verified.

## Phase mapping
- Phase 0: AI Hero, technical-writing, Diagram Design.
- Phases 1–2: rust-expert, rust-cli-clap, security code review and TDD.
- Phases 3–4: rust-tui-ratatui, terminal interaction tests, experience checks.
- Phase 5: search/knowledge, technical-writing.
- Phase 6: workflow/state-machine engineering, Rust tests.
- Phase 7: cross-platform release/packaging and portability audit.
- Phase 8: security, authorized remote transports, database adapters.
- Phase 9: semgrep + dependency reviews + full acceptance gates.

## Proposed local project skills (create only after underlying contracts exist)
- pocketops-tool-development → rename to **nyra-tool-development**; standardized registry/tool implementation.
- pocketops-tui-ux-review → **nyra-tui-ux-review**; task-based terminal QA.
- pocketops-portability-audit → **nyra-portability-audit**; OS, packaging, USB mode.
- pocketops-release-verification → **nyra-release-verification**; smoke/security/release checklist.

These four skills are now authored under `.agents/skills/` from NYRA's product, architecture, UX, portability, and release documents, and copied to the user's global skills directory. Their descriptions restrict automatic use to NYRA work. Update them when the underlying contracts change.

## Skill safety
Read remote skill source and scripts; avoid unattended install from arbitrary repos, tool execution with high OS privileges, persistent secrets, or hidden network actions. Pin reviewed revisions and document sources/versions. The required Diagram Design Skill was not available to the documentation authoring session; architectural diagram work remains pending.
