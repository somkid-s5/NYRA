# NYRA — Workflows and Incident Mode

## Purpose

Workflows combine registered, read-only diagnostics into a repeatable sequence. The CLI and TUI use NYRA's shared tool executor, so a workflow does not implement a second copy of network or system checks.

## Implemented workflow support

NYRA currently provides two workflow paths:

- **Web Diagnostics**, the built-in `web-unreachable` flow, available in the CLI and TUI.
- **Custom sequential workflows**, defined as typed TOML files and validated or executed from the CLI.

The custom workflow file format is versioned and validated against NYRA's trusted tool catalog. A workflow can declare typed inputs, bind a tool parameter from an input, a literal, or a typed scalar output from an earlier step, and conditionally run a later step when an earlier step has a matching status or finding. Workflow execution is sequential, with an overall timeout, per-step timeout, cancellation, and a bounded result. It accepts only registered read-only tools. The runner's JSON result uses schema version 1 and contains step summaries, not tool output payloads or workflow input values.

Validate a definition without running a diagnostic:

```sh
nyra workflow validate examples/workflows/local-system-health.toml
```

Run a validated definition through NYRA's shared executor:

```sh
nyra workflow run-file examples/workflows/local-system-health.toml --output text
nyra workflow run-file examples/workflows/local-system-health.toml --output json
```

`--input NAME=VALUE` may be repeated for declared workflow inputs. NYRA parses each value according to its declared type. For example, a workflow that declares a required URL input named `target` can be run with `--input target=https://example.com`. Undeclared or duplicate inputs are rejected.

The trusted tool registry uses these value types: `string`, `dns_hostname`, `hostname`, `ip_address`, `url`, `file_path`, `record_types`, `port`, `timeout_seconds`, `count`, `integer`, and `boolean`. NYRA performs type-specific semantic validation—such as hostname, IP, URL, record-type, and numeric-range checks—before execution.

The file size and definition shape are bounded. The schema rejects unknown fields, unsupported versions, free-form expressions, references to later steps, invalid type bindings, and tools outside the read-only catalog. Workflow files do not contain executable commands or shell strings.

### Example: local system health

The bundled [local-system-health.toml](../examples/workflows/local-system-health.toml) runs `system.info`; it runs `system.cpu` only if the first step completed. Both diagnostics use the same registered implementations as the direct CLI and TUI tools.

Three more ready-to-run examples are included and validated in CLI integration tests:

- [network-diagnostics.toml](../examples/workflows/network-diagnostics.toml) resolves one DNS name, then checks one authorized TCP port only when DNS returns an answer.
- [certificate-inspection.toml](../examples/workflows/certificate-inspection.toml) inspects TLS certificate details and system trust for one endpoint.
- [database-port-check.toml](../examples/workflows/database-port-check.toml) checks TCP reachability to a configured database port; it does not authenticate or test database protocol health.

Run any example with `nyra workflow run-file <PATH> --output text|json`. Network workflows should target only systems the operator is authorized to inspect.

### Definition structure

The workflow schema uses TOML tables and arrays of tables. `run_if` contains one shallow `all` or `any` group. Predicates can inspect only earlier steps:

```toml
[[steps]]
id = "system"
tool = "system.info"

[[steps]]
id = "cpu"
tool = "system.cpu"

[steps.run_if]
operator = "all"

[[steps.run_if.predicates]]
kind = "step_status"
step_id = "system"
equals = "completed"
```

Inputs are declared as `[[inputs]]` entries with `name`, `value_type`, optional `required`, and optional `default`. Tool parameters are declared under a step's `[[steps.parameters]]` array. Each binding's `source` is tagged `input`, `literal`, or `step_output`. The Rust schema and validator are documented in [`workflow_definition`](../src/workflow_definition.rs); the runtime's safe scalar input/output projection is in [`workflow_registry`](../src/workflow_registry.rs).

## Built-in Web Diagnostics

The CLI and TUI expose the built-in `web-unreachable` flow as **Web Diagnostics**. It validates one HTTP or HTTPS URL, resolves the hostname when applicable, checks TCP connectivity, inspects TLS for HTTPS, and sends an HTTP request only after earlier required checks pass. The TUI search query `web diagnostics` opens the same typed URL form.

```sh
nyra workflow run web-unreachable --target https://example.com --timeout 15 --output json
```

### Web Diagnostics report export

The Web Diagnostics CLI supports explicit sanitized JSON, HTML, or CSV report files:

```sh
nyra workflow run web-unreachable --target https://example.com --report-json ./web-report.json
nyra workflow run web-unreachable --target https://example.com --report-html ./web-report.html
nyra workflow run web-unreachable --target https://example.com --report-csv ./web-report.csv
```

Choose at most one report format per run. A report is separate from stdout and is written only when requested. A relative destination is resolved from the current working directory; the parent directory must already exist. NYRA writes through a temporary file in that directory, caps each final report at 64 KiB, and refuses to replace an existing destination. Invalid workflow input creates no report. Existing destinations fail with exit code 2; report destination or write failures use exit code 3.

JSON schema version 1 contains report metadata, an allowlisted workflow summary and step outcomes, HTTP status code when available, and a stable error code. HTML and CSV use a sanitized common metadata projection. Reports redact the target, omit observer identity and raw result/step data, and exclude DNS answers, selected peer addresses, TLS names, response headers, redirect destinations, and secret-bearing inputs. The `--output json` stdout result is a separate contract from the exported report.

Direct diagnostic commands and custom workflow files accept the same report options:

```sh
nyra sys info --report-json ./system-report.json
nyra workflow run-file examples/workflows/local-system-health.toml --report-html ./health-report.html
```

Custom workflow reports identify steps by their one-based position. They redact workflow and step IDs, omit inputs and raw tool data, and include only execution status, findings, timings, and fixed explanations for step issues. The result screen in the TUI also exports displayed diagnostic results in all three formats. Press Ctrl+E, choose a format, and enter a destination path.

## Detailed Web Diagnostics flow

1. Validate the URL, scheme, hostname, and absence of embedded credentials.
2. Resolve a hostname through the system resolver. An IP literal skips name resolution.
3. Connect to the effective URL port with a bounded timeout.
4. For HTTPS, inspect the peer certificate and report system trust validation. Plain HTTP skips TLS.
5. Send an HTTP request only after successful TCP and applicable TLS checks. Do not follow redirects or read a response body.
6. Summarize observed results and suggest scoped follow-up checks; do not assert a root cause from insufficient evidence.

## Planned capabilities

These ideas remain planned and are not provided by the current custom workflow CLI:

- A TUI workflow builder for browsing templates, editing steps, and launching custom definitions.
- Parallel DAGs, loops, nested conditions, free-form expressions, and retries.
- Authenticated profiles or secret-bearing workflow inputs.
- Custom incident templates and automated repairs.
- Opening, running, and exporting results from custom workflow files in the TUI.

Initial template ideas include server health, database connectivity, certificate audit, SFTP connectivity, Linux disk investigation, and network diagnostics. They must be implemented and tested before they are described as available.

## Incident Mode

The v1 incident experience is intended to guide users from a symptom—such as Website Unreachable, Connection Timeout, Server Slow, Disk Full, Database Connection Failed, or Certificate Problem—to relevant diagnostics, observed facts, and documented next checks. It will remain read-only; automated remediation is out of scope.
