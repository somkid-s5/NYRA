# NYRA Phase 1 review remediation

## Problem Statement

NYRA's Core 12 implementation exposes a shared Tool Registry, a CLI, and a guided TUI. Review of commit 045c6c2 against 2ad1c6e identified cases where a diagnostic run can outlive cancellation, contact a port different from the URL, or return a misleading status. Operators and scripts need trustworthy results from the same diagnostic tools. The expanded slice also lacks a passing build and test record for the reviewed commit.

## Solution

Correct the reviewed behavior in the shared implementation, preserve the existing CLI contract, and demonstrate the fixes through the compiled CLI and guided TUI. Keep the remaining first-release capabilities and release gates tracked in #1.

## User Stories

1. As an operator, I want SHA-256 to accept regular files and reject unsupported file types promptly, so that I can calculate a digest without tying up a worker.
2. As a TUI user, I want to cancel a file diagnostic and start another, so that I can recover without restarting the terminal.
3. As an operator, I want NYRA to check the opened file handle, so that the result describes the file actually opened.
4. As a TLS operator, I want an HTTPS URL and a separately supplied port to agree, so that the check contacts my intended endpoint.
5. As a TLS operator, I want IPv6 literals in HTTPS URLs to be accepted, so that I can verify certificates on IPv6 services.
6. As a TUI user, I want conflicting input to focus the correct field, so that I can fix it regardless of how the error message is worded.
7. As a scripter, I want a timed-out TCP connection attempt to produce its diagnostic finding even at the maximum allowed timeout, so that my automation handles it consistently.
8. As a scripter, I want a run that reaches its deadline before a finding to return a distinct result, so that I can tell incomplete runs from connection observations.
9. As a scripter, I want existing TCP JSON fields and exit codes to retain their meaning, so that existing automation remains compatible.
10. As an operator, I want results to identify the machine that made the observation, so that I can distinguish an SSH-host result from one produced locally.
11. As an operator, I want Disk Usage to explain when no volume data is available, so that I can tell missing data from measured values.
12. As an operator, I want unreadable measurements distinguished from zero bytes, so that I can tell unavailable information from a full disk.
13. As a TUI user, I want English search aliases and usable Unicode input fields, so that discovery follows the product language without losing support for my file paths.
14. As a release reviewer, I want format, lint, and test evidence for the expanded implementation, so that I can make a merge decision with a reproducible record.
15. As a maintainer, I want build prerequisites and any blocked platform gates recorded accurately, so that I can reproduce checks and assess release readiness.

## Implementation Decisions

- Route fixes through the existing Tool Registry and executor. Keep the CLI and TUI on the same tool implementations.
- Restrict SHA-256 to regular files and verify the opened handle. Use an open strategy that avoids waiting on unsupported special files and remains safe if the path changes between checks. Preserve bounded work and cancellation.
- Parse an HTTPS URL into a typed host and effective port. Treat an omitted HTTPS port as 443. Accept the URL's valid IPv6 literals and reject a conflicting port before a diagnostic run starts. Preserve hostname verification. The endpoint-preservation decision for the future Web Diagnostics workflow remains in effect.
- Use a consistent deadline policy so an attempted TCP connection that times out yields a completed finding with exit 1, and a run whose budget expires before it can begin or produce an observation yields exit 4. Preserve the existing TCP JSON v1 field semantics.
- Attach the observing machine's identity to the live result and present it in the TUI. Preserve compatibility for existing consumers and apply the established redaction policy to any future persisted data.
- Return explicit availability for disk observations. Report an empty or wholly unavailable result as such and preserve measurements for other volumes when one fails.
- Use the approved English search vocabulary and retain Unicode editing and paths. Use structured field associations for validation instead of selecting a field from message text. Consolidate the shared Ping process setup if it remains a small change alongside this work.
- Validate committed Rust CI on Windows and Ubuntu. Record native TLS dependency build prerequisites and resulting gate status. Local format success alone provides format evidence.

## Testing Decisions

- Extend the existing subprocess tests as the main seam for CLI behavior. Assert parsed JSON, exit codes, stdout/stderr, useful error messages, regular-file digest results, TLS endpoint validation, and IPv6 URL handling.
- Use the public shared executor and controlled fixtures only for cases the CLI cannot assert reliably, including deadline ordering, cancellation, and unavailable disk measurements. Keep real public network services out of regression tests.
- Use existing TUI TestBackend coverage for English search, structured error focus, and observer text. Verify terminal responsiveness and cancellation through the terminal entry point where supported.
- Check the SHA-256 file-type policy and bounded cancellation with controlled platform fixtures. Confirm that the caller can still run another diagnostic afterward.
- Run format, Clippy, and tests for the expanded slice. Record Windows and Ubuntu CI results separately. Report a build prerequisite failure separately from a failing application regression.

## Out of Scope

The first Web Diagnostics CLI/TUI path now exists as an initial workflow slice, and its current Windows x64 GNU build/test gate passes locally. The broader workflow engine remains open. Portable workspace and read-only USB behavior, report export and history, custom palettes and persistent appearance settings, release archives, and clean-machine Windows USB and Linux SSH validation also remain in #1. This spec does not replace their acceptance gates.

## Further Notes

The review initially observed format and diff checks passing, while Clippy and tests stopped because the Windows GNU host lacked GCC. On 2026-11-18, local verification was completed with Rust `stable-x86_64-pc-windows-gnu` and MSYS2 MinGW: Clippy passed with warnings denied, and all 38 unit plus 16 CLI integration tests passed. GitHub CI, a separate MSVC build, clean Windows/Linux hosts, and platform release smoke tests remain pending.

Follow-up verification on 2026-11-18 passed format, Clippy, and all tests on Windows x64 GNU (38 unit, 18 CLI) and Kali Linux under WSL (38 unit, 20 CLI). The Linux suite includes a trusted-local-HTTPS workflow test using a process-local test CA fixture and CLI coverage for localhost A/AAAA records. No clean Ubuntu/Windows host run or hosted GitHub CI result has been recorded.
