# NYRA — Tool Catalog and Discovery Requirements

The 12 core tools below have shared-registry metadata and execution paths in the worktree. Custom workflow files can be validated and run sequentially through the same registered executor. On 2026-11-23, Kali WSL passed `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, and `cargo test --locked --all-targets` (124 unit, 33 CLI). Windows x64 GNU format, check, and Clippy pass, but the full test suite remains blocked at link time by AWS-LC/MinGW's undefined `nanosleep64` symbol from `pthread_time.h`. The MSYS2 mingw64 bin directory must be on `PATH` for the Windows GNU toolchain. Clean-host smoke tests are pending, and registry entries remain `NeedsSmokeTest`; these checks do not establish platform support. 'Native' means Rust implementation; 'Adapter' uses the local platform/external package; 'Remote' authenticates to approved targets. Native-first is a strong preference, not a guarantee that every diagnostic is universal or unprivileged.

## Core 12 (initial build)
| ID | Display name | Backend | Inputs | Output / validation |
|---|---|---|---|---|
| network.tcp | TCP Port Check | native | host, port, timeout | reachable, refusal/timeout, duration |
| network.dns | DNS Lookup | native | hostname, record types | A/AAAA/MX/CNAME/TXT answers or error |
| network.reverse_dns | Reverse DNS | native | IP | PTR records |
| web.http | HTTP Health | native | URL, timeout | status code, headers, measured elapsed |
| security.tls | TLS Certificate Inspect | native | host/URL, port | subject/SAN, issuer, validity/expiry |
| network.interfaces | Local Network Interfaces | OS | local | interface addresses, netmasks, loopback; no gateway/DNS data |
| network.ping | Ping | OS adapter | host, count, timeout | reachable/first latency when reported; system `ping` required and ICMP may be filtered |
| system.info | System Info | native | local | OS, arch, hostname, uptime if supported |
| system.cpu | CPU Information | native | local | CPU model/count/load/sampled utilization |
| system.memory | Memory Usage | native | local | total/used/available and units |
| system.disk | Disk Usage | native | local | mounted volume names, file systems, total and available bytes |
| file.sha256 | SHA-256 File Hash | native | path | SHA-256, file size, path validation |

## Workflow registry projection

The workflow catalog derives typed parameters from the registered input fields and uses the same parser/executor as direct diagnostics. Only read-only registered tools are accepted. Bindable outputs are an explicit scalar allowlist; raw collections, response headers, interface addresses, and identifying file paths are excluded. Tests cover registry/schema type mapping, output allowlisting, condition evaluation over earlier step statuses/findings, sequential execution, and that file paths or rejected URL credentials are not echoed in workflow results/errors. See [`workflow_registry`](../src/workflow_registry.rs) and the [workflow format and CLI](workflows.md#implemented-workflow-support).

User-facing commands for select tools are specified in docs/architecture.md and may evolve by documented ADR during Phase 0. Do not silently alter documented stable CLI names once released.

## Expanded network (v0.5-v0.8)
Traceroute; multi-port check with explicit bounds; network interface state; DNS troubleshooting; SFTP SSH handshake; Nmap external adapter with dependency detection; approved-scope host reachability. Nmap raw packet functionality (and Windows Npcap) cannot be promised zero-install.

## Expanded system (v0.5-v0.8)
Processes, listening sockets, service status by OS, environment checks, permissions, load metrics, file/FS inspections; Linux health via SSH, Windows service/Event inspection via approved transport.

## Security (v0.5-v1)
TLS chain vs expiration distinction; certificate audit, warning thresholds; HTTP security headers; hashes/file integrity; safe configuration checker; port exposure check only for explicitly authorized targets.

## Database (v0.8)
**Oracle**: port 1521 check, listener connectivity, (authorized) lsnrctl status, database login/session metrics, tablespace/datafile, archive log and alert log.
**SQL Server**: TCP 1433 check, protocol-level connection/login, database status and connection diagnostics with rights; test platform prerequisites.
A positive TCP response is **not** proof of database health, credentials or query privilege.

## Logs and knowledge
Local streaming log viewer, grep/regex-like search, severity/time filters, tail, count/group by error, context lines and safe export. Later parsers: IIS, WebLogic, Apache, Nginx, Oracle. Command cards for Linux, Windows, Solaris, Oracle, networking, certificates, storage.

## Registry metadata (minimum)
- id, version, English display name and description
- category, tags, English keywords and aliases, sample queries
- input schema and validator, helpful defaults and examples
- supported platforms, required privileges and optional dependencies
- execution backend and risk classification
- timeout, cancellation and maximum resource demand
- structured output schema and status explanation
- knowledge/help links and optional suggested follow-on checks

## Search algorithm (offline)
1. Normalize queries and handle Unicode safely.
2. Exact ID/command matches score first.
3. Match curated English aliases such as "check port", "port", and "telnet" → network.tcp.
4. Use fuzzy rank for typos and substring score; weight usage/favorites modestly.
5. Present top results with intent/category and readiness, not fabricated AI reasoning.
6. Offer suggestions when no match; allow editable keywords in user data later.

Examples:
- "disk full", "free disk space", "df -h" → system.disk + disk knowledge cards.
- "certificate expires", "TLS certificate expiry" → security.tls.
- "oracle listener" → Oracle category or port connectivity check based on availability.
- "website unreachable" → Web Diagnostics workflow and HTTP tool.

## Command knowledge card fields
Title, OS, intent, exact syntax, parameters, worked example, expected output, explanation, risk level, side effects, required privilege, related commands, and a safe copy action. Do not execute command-card text by default or accept arbitrary shell strings.

Example (read-only):
~~~sh
find /var/log -type f -size +100M
~~~
Use to locate files over 100MB under /var/log; not a delete/repair command.

The current offline bundle contains 19 Windows PowerShell, Linux, macOS, Solaris, and FreeBSD references for filesystem space, large-file discovery, name-service lookup, and TCP connectivity/status. Solaris cards cover `df -k`, bounded-root `find` usage under the usual `/var/adm` system-message and crash-dump directory, Solaris `-size` units, `getent hosts`, one explicitly authorized `nc -z` TCP connection attempt, and local `netstat` TCP socket status. FreeBSD cards cover filesystem/inode capacity via `df` and IPv4/IPv6 listening TCP sockets via `sockstat`; they are display/copy-only references, not evidence of native FreeBSD support. The `nc` package may be absent and the card does not claim a bounded connect timeout. `nyra knowledge list` lists cards, `nyra knowledge search check port` searches their curated metadata, and `nyra knowledge show <id>` displays the full reference. CLI search requires every query term to appear in card metadata and retains catalog order. In the TUI, F6 opens Command Knowledge; Ctrl+K opens global tool search. Knowledge search locally ranks curated card metadata and tolerates small typos. This is deterministic search, not general natural-language understanding. Tab filters card favorites, Ctrl+F saves/removes the selected card in the active settings file, and Ctrl+Y sends its static command text using OSC 52. Clipboard support depends on the terminal. Cards include intent, syntax, parameters, example, expected output, risk, side effects, required privileges, and related NYRA diagnostics. NYRA displays but never executes card commands. Further OS coverage remains planned.

Solaris syntax and behavior were checked against the [Oracle Solaris 11.4 filesystem guide](https://docs.oracle.com/cd/E37838_01/pdf/E61016.pdf) for `df -k`, [Oracle Solaris file-size guidance](https://docs.oracle.com/cd/E26502_01/html/E29006/sysresdiskuse-3.html) for the 512-byte `find -size` unit, [Oracle's Solaris 11.4 system-message documentation](https://docs.oracle.com/cd/E37838_01/html/E61000/gprvc.html) for `/var/adm`, [Oracle's Solaris `getent hosts` example](https://docs.oracle.com/cd/E19253-01/820-3508/6nf1ndpac/index.html), the [`netcat(1)` reference](https://docs.oracle.com/cd/E88353_01/html/E37839/netcat-1.html) for single-port `-z` behavior and the optional `network/netcat` package, and [Solaris `netstat` documentation](https://docs.oracle.com/cd/E37838_01/html/E60991/ipconfig-142.html) for local socket inspection. These cards are references only and do not establish Solaris binary or diagnostic-tool support.

## Saved tools: Favorites and Recent

In the TUI, Ctrl+F toggles the selected tool's favorite in the Tools screen. F5 opens Favorites/Recent; Tab switches between those lists, and Enter opens the selected tool form. Ctrl+R in Saved Tools toggles recent tracking; disabling tracking clears existing recent IDs and persists `[discovery].record_recent = false`, while enabling it starts with an empty list. The setting defaults to enabled for older configs without the field. A tool is added to Recent after a valid run starts while tracking is enabled. NYRA persists only stable tool IDs in `[discovery].favorite_tools` and `[discovery].recent_tools`, with at most 10 recent IDs; targets and input values are not recorded. Writes are asynchronous, and a save failure leaves the current session usable while reporting the failure. Ctrl+F does not act while editing form fields.

The macOS TCP and DNS syntax was checked against the Xcode manual pages for [`nc(1)`](https://keith.github.io/xcode-man-pages/nc.1.html) and [`dscacheutil(1)`](https://keith.github.io/xcode-man-pages/dscacheutil.1.html). This verifies reference syntax only; it does not establish NYRA runtime support on macOS.

## Tool quality gate
Every implementation ships help, supported-platform statement, typed validation, timeout, cancellation if relevant, structured result, no unbounded output by default, sensitive-data treatment and tests. If prerequisites absent, show "REQUIRES SETUP" and what is missing.
