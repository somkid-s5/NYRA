---
name: nyra-release-verification
description: Use in the NYRA repository when checking a release, roadmap phase gate, CI result, package, or readiness claim against release criteria. Ignore unrelated repositories.
metadata:
  version: "1.0.0"
---

# NYRA Release Verification

Use this skill only when the current repository is NYRA. Read `AGENTS.md`, `docs/testing-release.md`, `docs/roadmap.md`, and `docs/portability-security.md` before assessing a release or phase gate.

## Verify the release evidence

- Inspect the actual diff, build outputs, CI results, and user-facing flows. Do not accept a completion summary as test evidence.
- Check the gates applicable to the change: formatting, Clippy, tests, CLI output and exit contracts, TUI render or PTY behavior, security and redaction, workflow behavior, and target-platform smoke tests.
- Verify each advertised OS/architecture on a clean environment without Rust installed. Report missing targets as unverified.
- Inspect package contents, version, checksums, license notices, release notes, and installation/portable-mode instructions.
- Confirm no plaintext credentials, unlicensed assets, or unsupported compatibility claims are included.
- Keep phase status and README/roadmap claims aligned with completed evidence. A negative diagnostic finding is not an application failure; classify it using the documented result contract.
- Do not claim v1.0 readiness unless the security, UX, portability, and release criteria in `docs/testing-release.md` are all evidenced.

## Result format

Report each gate as Pass, Fail, Not Run, or Unknown, with the command or artifact that supports it. List blockers separately from items that were not tested. Never turn a skipped check into a pass.
