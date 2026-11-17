# NYRA — Tool Catalog and Discovery Requirements

All tools below are planned, not implemented. 'Native' means Rust implementation; 'Adapter' uses the local platform/external package; 'Remote' authenticates to approved targets. Native-first is a strong preference, not a guarantee that every diagnostic is universal or unprivileged.

## Core 12 (initial build)
| ID | Display name | Backend | Inputs | Output / validation |
|---|---|---|---|---|
| network.tcp | TCP Port Check | native | host, port, timeout | reachable, refusal/timeout, duration |
| network.dns | DNS Lookup | native | hostname, record types | A/AAAA/MX/CNAME/TXT answers or error |
| network.reverse_dns | Reverse DNS | native | IP | PTR records |
| web.http | HTTP Health | native | URL, timeout | status code, headers, measured elapsed |
| security.tls | TLS Certificate Inspect | native | host/URL, port | subject/SAN, issuer, validity/expiry |
| network.interfaces | Local Network Info | OS/native | local | interfaces, addresses, gateway/DNS if supported |
| network.ping | Ping | OS adapter/native only where supported | host, count, timeout | reachable/latency, privilege caveat |
| system.info | System Info | native | local | OS, arch, hostname, uptime if supported |
| system.cpu | CPU Information | native | local | CPU model/count/load/sampled utilization |
| system.memory | Memory Usage | native | local | total/used/available and units |
| system.disk | Disk Usage | native | local | mounts, free, usage %, errors |
| file.sha256 | SHA-256 File Hash | native | path | SHA-256, file size, path validation |

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
- id, version, display names/description (Thai and English)
- category, tags, bilingual keywords/aliases, sample queries
- input schema and validator, helpful defaults and examples
- supported platforms, required privileges and optional dependencies
- execution backend and risk classification
- timeout, cancellation and maximum resource demand
- structured output schema and status explanation
- knowledge/help links and optional suggested follow-on checks

## Search algorithm (offline)
1. Normalize query; preserve Thai script and handle Unicode safely.
2. Exact ID/command matches score first.
3. Match synonym/alias list incl. Thai: "เช็คพอร์ต", "ตรวจพอร์ต", "port", "telnet" → network.tcp.
4. Use fuzzy rank for typos and substring score; weight usage/favorites modestly.
5. Present top results with intent/category and readiness, not fabricated AI reasoning.
6. Offer suggestions when no match; allow editable keywords in user data later.

Examples:
- "disk full", "พื้นที่เต็ม", "df -h" → system.disk + disk knowledge cards.
- "certificate expire", "cert หมดอายุ" → security.tls.
- "oracle listener" → Oracle category or port connectivity check based on availability.
- "website เข้าไม่ได้" → web diagnostics workflow and HTTP tool.

## Command knowledge card fields
Title, OS, intent, exact syntax, parameters, worked example, expected output, explanation, risk level, side effects, required privilege, related commands, copy/run actions. Avoid "run arbitrary user-provided shell string" as a default capability.

Example (read-only):
~~~sh
find /var/log -type f -size +100M
~~~
Use to locate files over 100MB under /var/log; not a delete/repair command.

## Tool quality gate
Every implementation ships help, supported-platform statement, typed validation, timeout, cancellation if relevant, structured result, no unbounded output by default, sensitive-data treatment and tests. If prerequisites absent, show "REQUIRES SETUP" and what is missing.
