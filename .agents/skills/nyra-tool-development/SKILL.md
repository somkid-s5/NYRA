---
name: nyra-tool-development
description: Use in the NYRA repository when adding or changing a diagnostic tool, tool registry, typed inputs, shared executor, result contract, or tool-facing CLI, TUI, and workflow integration. Ignore unrelated repositories.
metadata:
  version: "1.0.0"
---

# NYRA Tool Development

Use this skill only when the current repository is NYRA. Read its `AGENTS.md` and the relevant requirements before changing code. NYRA is planning-only until working code and tests prove otherwise; do not treat planned commands, tools, binaries, or support claims as implemented.

## Project contracts

- Follow `docs/architecture.md`, `docs/product-requirements.md`, `docs/portability-security.md`, and the relevant tool entry in `docs/tool-catalog.md`.
- Keep CLI, TUI, and workflows on the same registry and executor. A UI must not own a separate implementation of diagnostic logic.
- Model tool metadata, typed inputs, capabilities, execution status, findings, warnings, and errors as described in the architecture document. Execution success and a negative diagnostic finding are separate outcomes.
- Default to read-only behavior and least privilege. Validate host, port, path, URL, and timeout; bound work and support cancellation where relevant.
- Treat input as untrusted. Do not interpolate it into shell command strings. Detect optional dependencies and report when they are unavailable.
- Distinguish locked requirements from recommendations and future ideas. Record a meaningful architecture decision in `docs/decisions.md` when changing a locked contract.

## Working approach

Choose the smallest verified step in `docs/roadmap.md`, implement one useful vertical slice, and add tests for its requirements. Keep the shared tool contract stable across interfaces. Update documentation only to describe behavior demonstrated by code and test output.

Before reporting completion, state what changed, which checks actually ran, and any platform or capability limits. Never report a planned feature or an unrun test as complete.
