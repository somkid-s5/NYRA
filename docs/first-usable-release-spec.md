# NYRA first usable release specification

## Problem Statement

System administrators need to answer questions such as “Can this machine reach that port?”, “Why won't this website load?”, and “Which disk is full?” from whichever machine they are using. The available utilities vary by host, and remembering command syntax slows down troubleshooting. The existing NYRA TCP CLI proves one diagnostic on a Windows development machine. Operators still need a discoverable catalog, a workflow that explains which layer failed, and verified binaries they can carry or run in an SSH session.

## Solution

Ship the first usable release of NYRA as an English-language, offline-first toolkit. Expose the Core 12 diagnostics through a searchable keyboard TUI and a scriptable CLI. Provide a bundled Web Diagnostics workflow that takes one URL and shows what happened at DNS, TCP, TLS, and HTTP. Package Windows and Linux x64 binaries and verify them on clean machines. Share tool execution and diagnostic findings across all three entry points.

## User Stories

1. As a system administrator, I want to launch NYRA from a Windows USB drive, so that I can troubleshoot on a machine without Rust or Cargo.
2. As a Linux operator, I want to run NYRA in an existing SSH session, so that I can observe connectivity from the server's perspective.
3. As a terminal user, I want the TUI to work at 80×24, so that a small terminal can still show forms and results.
4. As a keyboard user, I want discoverable shortcuts and visible focus, so that I can complete a diagnostic without a mouse.
5. As a system administrator, I want to search for “check port” or “disk space,” so that I can find the right tool without knowing its name.
6. As a user who mistypes a search, I want fuzzy matches after exact names and aliases, so that I can still find the tool.
7. As a TUI user, I want English labels, help, and results, so that I see consistent terminology throughout the application.
8. As a TUI user, I want form errors to keep my entries and focus the invalid field, so that I can correct input quickly.
9. As an operator, I want to see the target and the machine making the observation, so that I can interpret the finding in context.
10. As an operator, I want to cancel a running diagnostic, so that I can recover from a slow check while keeping any completed observations.
11. As a terminal user, I want normal terminal behavior restored after quitting or interrupting NYRA, so that I can resume work in my shell.
12. As an operator, I want to check TCP connectivity to one host and port, so that I can distinguish acceptance, refusal, timeout, unreachable, and inconclusive results.
13. As an operator, I want DNS Lookup to show records for a hostname, so that I can inspect its published DNS data.
14. As an operator, I want Reverse DNS to show PTR results for an IP address, so that I can investigate its reverse name.
15. As an operator, I want HTTP Health to report status, elapsed time, and selected safe headers, so that I can diagnose an application's HTTP response.
16. As an operator, I want TLS Certificate Inspect to report certificate identity, expiry, and verification, so that I can investigate HTTPS failures.
17. As an operator, I want to inspect local network interfaces and addresses, so that I can understand which interfaces are available.
18. As an operator, I want Ping to report reachability where supported, so that I can make a bounded reachability check with clear platform limitations.
19. As an operator, I want basic system information, so that I can identify the host and its operating environment.
20. As an operator, I want CPU information, so that I can assess processor capacity and observed utilization.
21. As an operator, I want memory totals and availability, so that I can investigate memory pressure.
22. As an operator, I want disk usage by volume, so that I can find low space and see unavailable measurements.
23. As an operator, I want to compute a file's SHA-256 digest, so that I can compare it to an expected checksum.
24. As an operator, I want unavailable tools to explain what is missing, so that I can choose another check or prepare the environment.
25. As a scripter, I want argument validation and stable exit codes, so that I can distinguish findings from invocation and execution failures.
26. As a scripter, I want a single versioned JSON result on stdout, so that I can parse it reliably.
27. As an operator who uses both interfaces, I want equivalent diagnostic findings in the CLI and TUI, so that either interface can verify the other.
28. As a web operator, I want one Web Diagnostics entry point, so that I can see DNS, TCP, TLS, and HTTP observations together.
29. As a web operator, I want the workflow to test the same selected destination through later protocol layers, so that the observations identify the same endpoint.
30. As a web operator, I want skipped steps and their reasons, so that an upstream failure does not look like multiple unrelated failures.
31. As a web operator, I want an HTTP redirect reported separately, so that I can decide whether to check the new URL.
32. As a web operator, I want next-check suggestions grounded in observations, so that I can investigate without treating a guess as a cause.
33. As an operator, I want to export a diagnostic report explicitly in JSON or HTML, so that I can share the evidence in an appropriate format.
34. As an operator working with internal systems, I want sanitized exports and opt-in history, so that I can avoid exposing credentials or private targets.
35. As a USB user, I want portable settings to travel with the binary, so that I can reuse my preferences across hosts.
36. As a USB user, I want diagnostics to run on read-only media with a clear indication that settings cannot be saved, so that I can still troubleshoot.
37. As a TUI user, I want the four built-in themes and a validated custom palette, so that I can choose a readable appearance.
38. As a TUI user, I want to disable sound and motion, so that the interface works in quiet or reduced-motion environments.
39. As a release user, I want archives and checksums for supported platforms, so that I can select and verify the correct download.
40. As a release reviewer, I want clean-machine and terminal test evidence, so that I can assess portability claims before publication.

## Implementation Decisions

- Keep one Rust package with shared typed tool dispatch. The registry supplies stable tool IDs, English metadata, input fields, aliases, capabilities, and risk ratings. CLI, TUI, and workflows call the same executor.
- The executor validates input and capabilities, bounds runtime and resource use, propagates cancellation, and returns a diagnostic run result. A diagnostic finding describes the observation, while run status describes whether execution completed.
- Preserve the TCP JSON v1 contract. Use typed tool-specific data, explicit units and stable fields, and bump the schema version for breaking changes. A started check has one JSON result on stdout. Usage errors leave stdout empty. Send progress and diagnostics to stderr.
- Preserve exit semantics: 0 for a requested positive finding or measurement, 1 for a completed negative or inconclusive finding, 2 for invalid input, 3 for execution failure, 4 for a run deadline before a finding, and 130 for cancellation. A timed-out TCP attempt is a diagnostic finding.
- Use a responsive Ratatui interface with search, forms, progress, results, settings and scoped help. Keep execution off the terminal event loop. Search uses stable ranking and English aliases. Preserve Unicode path handling.
- Compile the initial workflow from a bundled definition. Reject URL credentials and unsupported schemes. Resolve through the system resolver where applicable. Retain the selected TCP address through TLS and HTTP while preserving the original hostname for verification and routing. Display redirects for explicit follow-up. Require successful applicable TLS verification before HTTP.
- Use read-only diagnostics, explicit single targets, bounded operations, and typed platform adapters. Report missing capabilities with an actionable reason. Keep saved settings separate from history and reports. History starts disabled. Apply shared redaction to persisted reports and allow the operator to review what is shared.
- Use an executable-relative portable workspace selected by its marker. In read-only portable mode, keep running diagnostics and keep host-profile writes out of the portable session.
- Ship four data-driven palettes, validate custom settings before applying them, and support focus and status cues that do not depend on color. Optional sound and motion stay quiet in CLI and SSH or headless use.
- Complete shared TCP infrastructure and CI first, followed by DNS, TLS, HTTP and the Web Diagnostics workflow, then the remaining Core 12 and TUI. Verify clean-machine packaging before calling the release usable.

## Testing Decisions

- Use shipped entry points as the main testing seam. Run the compiled CLI as a subprocess and assert documented exit codes, stdout/stderr, parsed JSON and diagnostic findings. Extend the existing TCP integration tests, which already use loopback for accepted and refused connections and cover invalid input.
- Run the bundled workflow through its CLI entry point against controlled local endpoints. Assert DNS and TCP findings, valid and invalid TLS, HTTP failure and redirect observations, skipped steps, cancellation, and consistent endpoint attribution. Use deterministic fixtures only for conditions such as resolver outcomes that cannot be controlled reliably through a real local endpoint.
- Drive the three operator journeys through a real TUI terminal session: find a TCP check from an English query, check disk space, and identify the failing Web Diagnostics step. Compare findings with the CLI. Use Ratatui TestBackend for layout and focus states that cannot be proved reliably through one terminal transcript.
- Verify that settings, history, and exports respect the storage boundary. Check read-only and writable USB media and assert that redaction removes credentials and unselected private data. Inspect sanitized HTML and JSON as an operator would.
- Run format, lint, and tests on Windows and Ubuntu CI. Smoke-test packaged binaries on clean Windows and Linux without Rust or Cargo. Verify the Windows USB launch and the Linux SSH TUI at 80×24. Independently review the user journeys, test output, and release artifacts before publishing.
- Assert observable output and behavior. Existing TCP tests and simple local fixtures are the precedent. Keep private function structure out of acceptance tests.

## Out of Scope

The first usable release excludes NYRA-initiated SSH and Windows remote sessions, authenticated database health, general-purpose workflow authoring, enhanced command knowledge and incident mode, ARM64 and additional release targets, automatic repairs, offensive tooling, mandatory accounts or AI, and additional app languages. These remain later roadmap items where applicable.

## Further Notes

This is the first usable release spec. The end-to-end design also describes how knowledge cards, richer workflows and exports, authenticated read-only adapters, and evidence-based incident mode can mature toward v1.0. Local Windows x64 GNU TCP checks have passed. GitHub CI, the clean-machine release matrix, and the operator journeys remain acceptance gates.
