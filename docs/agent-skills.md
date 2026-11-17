# NYRA — Curated AI Agent Skills and Agent Workflow

## Purpose
Use a lean, high-signal set of skills so Codex Cloud can deliver high-quality Rust CLI/TUI code, meaningful testing and UX review. Gemini may serve as an independent reviewer. Skills augment instructions but do not override code/tests/security requirements in AGENTS.md.

**Status disclaimer:** These are previously recommended repositories and skill names from project planning. They are **recommendations, not verified installed skills**; URL structure, name/availability/compatibility and third-party security must be checked before installation, especially inside Codex Cloud. Do not claim any are currently active.

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

## Candidate setup (PowerShell; syntax must be verified in destination environment)
~~~powershell
cd C:\AIWorkspace\NYRA

npx skills add BjornMelin/dev-skills -a codex -a gemini-cli -s rust-expert -s rust-cli-clap -s rust-tui-ratatui -s technical-writing
npx skills add ggonzalez94/skills -a codex -a gemini-cli -s agent-cli-design
npx skills add semgrep/skills -a codex -a gemini-cli -s code-security
npx skills add cathrynlavery/diagram-design -a codex -a gemini-cli
npx skills list
~~~
The snippet is not proof that any skill was installed; Codex Cloud may have a different skill location or execution environment than the Windows workstation. Check command support and permissions before running. AI Hero may already be installed locally; avoid duplicates. Semgrep analysis also requires a functioning scanner, not just the instruction skill.

## Preferred roles
- **Codex Cloud / Implementer:** reads AGENTS.md and Product Spec, chooses a small ticket, writes code/tests, runs and reports test results, makes reviewable changes.
- **Gemini / Reviewer:** checks design compliance and actual diff, probes UX, handles alternate solutions, tests quality/security; no rubber-stamping.
- **Verifier / Release Gate:** checks target OS compatibility, CLI behavior, saved artifacts, changelog and traceability. May be same person/tool only if independently verified.

## Phase mapping
- Phase 0: AI Hero, technical-writing, Diagram Design (when available).
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

These custom skills are **future deliverables** and have not been authored/installed yet. Put agent instructions in appropriate skill directories only after verifying the target agent's discovery convention.

## Skill safety
Read remote skill source and scripts; avoid unattended install from arbitrary repos, tool execution with high OS privileges, persistent secrets, or hidden network actions. Pin reviewed revisions and document sources/versions. The required Diagram Design Skill was not available to the documentation authoring session; architectural diagram work remains pending.
