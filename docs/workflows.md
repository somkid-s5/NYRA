# NYRA — Workflow Engine and Incident Mode

## Purpose
Combine multiple read-only diagnostic tools into one repeatable execution plan with shared inputs, per-step timing/status and evidence-based summary. Workflows must run in both CLI and TUI over the common Tool Executor.

## Engine minimum
- Serializable workflow definition, unique ID/version, description, input schema and ordered steps.
- Each step references a registered tool ID with typed parameter mapping.
- Shared variables (target hostname, URL, port, authenticated profile references).
- Step dependencies and limited branching: run if previous status/finding matches condition.
- Cancellation, bounded concurrency, timeout per step and overall budget.
- Recoverable errors with clear stopped/skipped/failed/completed state.
- Same structured result schema as direct tools, with aggregated report.
- Default read-only; authorization guard for higher-risk actions in future.
- Explicit redact-before-persist policy for workflows and reports.

## Initial workflow templates
| ID | Flow |
|---|---|
| web-unreachable | URL parse → DNS → TCP port → TLS (HTTPS) → HTTP |
| server-basic-health | OS Info → CPU → Memory → Disk (local initially, SSH later) |
| database-connectivity | DNS → TCP → optional protocol/auth check |
| certificate-audit | TLS handshake → subject/SAN/issuer → expiry & chain |
| sftp-connectivity | DNS → TCP SSH port → optional SSH handshake |
| linux-disk-investigation | FS free → inode usage → large files → logs |
| network-diagnostics | DNS → reachability → TCP → HTTP where applicable |

Only 'web-unreachable' is required for first workflow slice. All others are planned.

## Detailed web-unreachable flow
Input: URL, timeout and optional max redirects.
1. Parse/validate URL; reject unsupported scheme, invalid host and unexpected credential-bearing URL in exported report.
2. Resolve host with DNS; record answers and DNS failure.
3. Establish TCP connection to URL port (default 443 HTTPS, 80 HTTP), with bounded timeout.
4. For HTTPS, perform TLS handshake, read certificate and capture expiry/verification outcomes.
5. Perform HTTP request, record status, latency and headers within bounded limits.
6. Summarize only observed results, e.g. DNS/TCP/TLS passed but HTTP 503 indicates application/upstream investigation.
7. Offer **next check** suggestions with explicit confidence/scope; do not assert an exact root cause with insufficient evidence.
8. Export user-selected report (HTML/JSON; CSV for tabular result sets).

## Illustrative workflow file (not yet executable)
~~~toml
id = "web-unreachable"
version = 1
description = "Diagnose unreachable website"
risk = "read-only"

[inputs]
target = "url"
timeout_seconds = 5

[[steps]]
id = "dns"
tool = "network.dns"

[[steps]]
id = "tcp"
tool = "network.tcp"
run_if = "dns.completed"

[[steps]]
id = "tls"
tool = "security.tls"
run_if = "tcp.finding == open and scheme == https"

[[steps]]
id = "http"
tool = "web.http"
run_if = "tcp.finding == open"
~~~
Schema is conceptual and must be validated/compiled into a strongly typed workflow model by implementation; do not execute a free-form expression via eval.

## Builder UX
Template Library → inspect steps → choose inputs and safe conditions → save custom workflow → run → step progress → result summaries. Initial builder limited to sequential steps and simple conditions; parallel DAGs later.

## Incident Mode (v1)
Select symptom (Website Unreachable, Connection Timeout, Server Slow, Disk Full, Database Connection Failed, Certificate Problem) → appropriate diagnostics template → observed facts → next checks with documented rules. Not mandatory AI. No automated repairs or mutations.

## Reports and history
Reports should include run identifier, timestamp/offset, target display with redaction, platform, step outputs, diagnostics, summary, failures, warnings, version, verification limits. Export requires explicit action; share sanitization helper across CLI and UI.
