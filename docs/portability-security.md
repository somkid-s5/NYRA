# NYRA — Portability, Platforms, Privacy and Security

## Portable usage contracts
**GitHub Releases:** provide separately compiled archives for supported OS/CPU targets. User extracts archive and runs executable; no Rust or Cargo on target. Windows from directory: \`.\nyra.exe\`; Linux from directory: \`./nyra\` with executable permissions.

**USB:** executable + config + themes + profiles + recipes + data/reports can live on removable drive; moving USB preserves settings where writable. Use a \`portable.flag\` marker (proposed) to select workspace paths and avoid using host profile by default. Handle read-only media without crashing.

**Git clone:** for development; does not imply target computer has compiler. Do not require admin rights for ordinary diagnostics.

**Distribution structure (concept only):**
~~~text
NYRA-Portable/
  windows/nyra.exe
  linux-x64/nyra
  linux-arm64/nyra
  portable.flag
  config/config.toml
  themes/custom.toml
  profiles/
  recipes/
  data/
  reports/
~~~
Packaged asset paths must resolve correctly on removable drives with spaces or Unicode filenames.

## Compatibility goals / no unconditional promises
| OS | Approach / status |
|---|---|
| Windows 11 x64 | primary, test on clean host |
| Windows 10 x64 | targeted where supported, validate version |
| Windows Server 2022 | targeted smoke tests |
| Ubuntu 22.04/24.04 | primary Linux validation |
| RHEL 8/9 | targeted compatibility tests |
| Linux ARM64 | separate build after verification |
| macOS Intel/Apple Silicon | future separate builds |
| Solaris 11.4 SPARC | agentless **remote SSH**, not guaranteed native Rust executable |
| Windows Server 2008 R2 | no native support promise |

A single binary cannot run unchanged on every OS/architecture. Linux musl/static linking may help but TLS, DNS, audio, OS APIs and C dependencies still require actual target validation. Nmap on some Windows/raw-packet uses may require Npcap. Terminal output/font capabilities also vary.

## Capability model
Native Rust: minimal deps and privilege where feasible. OS Adapter: only where OS supports it. External Adapter: detect Nmap/others, show Ready/Requires Setup, do not silently install. Remote Adapter: use authenticated/authorized SSH/WinRM or protocol-specific libraries; never assume same command set across Windows/Linux/Solaris.

## Security rules
- Default to read-only local/remote diagnostics; no auto-repair or stealth scans.
- Respect explicit authorized target scope. Add concurrency caps, per-target timeouts and confirmation for broad scans.
- Least privilege; do not request admin/root automatically for core use.
- Treat all user-provided hosts, paths, URLs, regex patterns and command arguments as untrusted.
- No interpolation into a shell command string. Use native APIs or Process Command arg vector. Restrict custom recipes and external adapters.
- Validate URL schemes/addresses; guard against SSRF and local/private target surprises in automated workflows.
- Authenticate remote connections; check SSH host key; validate remote TLS certificate/WinRM auth.
- Never write plaintext passwords/tokens, SSH keys or database secrets to config/reports/history. Prefer OS store/SSH agent/secure prompt.
- Redact logs, command output and report targets when sensitive; default report saving to explicit action.
- Verify downloaded releases by checksum/signature where supported; document supply chain.
- Do not bypass endpoint execution controls, managed workstation policies or enterprise security products.

## Risk levels
- R0: local metadata/read-only diagnostics, minimal resource use.
- R1: network contact to explicitly named authorized target or bounded probes.
- R2: privileged read-only remote checks requiring authentication.
- R3: state-changing/destructive action, excluded from early releases; must require preview and explicit user confirmation later.

## Operational controls
History toggle/clear, max entries, local data location, offline core behavior, redaction toggle, log verbosity, maximum scans and timeouts, safe cancellation and permissions messages.
