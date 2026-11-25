---
name: nyra-portability-audit
description: Use in the NYRA repository when evaluating supported platforms, release targets, dependencies, USB portability, or clean-machine behavior. Ignore unrelated repositories.
metadata:
  version: "1.0.0"
---

# NYRA Portability Audit

Use this skill only when the current repository is NYRA. Read `AGENTS.md`, `docs/portability-security.md`, `docs/testing-release.md`, and the relevant roadmap phase before making a compatibility or portability claim.

## Audit against evidence

- Treat the OS list as compatibility goals until each target has a clean-environment build and smoke test. A successful local build does not establish support.
- Check Windows and Linux artifacts separately, including architecture, runtime dependencies, executable permissions, and documented platform differences.
- For portable mode, inspect path resolution on removable media, Unicode and spaced paths, writable and read-only media behavior, and whether host configuration is changed.
- Confirm ordinary diagnostics do not require administrator/root privileges or a Rust toolchain on the target machine.
- Identify optional external tools and report them as Ready or Requires Setup. Do not silently install them or claim zero-install support without evidence.
- Check timeout and resource bounds, least privilege, authenticated remote operations, host-key or certificate validation, and secret redaction against the project security contract.
- Do not claim one binary works across different operating systems or architectures. Do not claim old Windows, Solaris-native, or other future targets without explicit passing evidence.

## Report

For each target, report the exact build and smoke-test evidence, dependencies, known limitations, and whether the claim is verified, planned, or unknown. Do not edit the compatibility matrix to imply support that has not been demonstrated.
