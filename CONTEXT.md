# NYRA Diagnostics

NYRA helps system administrators run and interpret diagnostic checks from the machine where they are troubleshooting.

## Language

**Diagnostic tool**:
A named check that answers a specific troubleshooting question, such as whether a target accepts a TCP connection from the machine running NYRA.

**Diagnostic run**:
One invocation of a diagnostic tool or workflow from a particular machine against a defined target.

**Diagnostic workflow**:
A set of related diagnostic tools that answer a broader troubleshooting question from shared input. Web Diagnostics uses a URL to check which parts of reaching a website work from the machine running NYRA.

**Diagnostic finding**:
An observation from a diagnostic tool. For example, a refused TCP connection is a finding about the attempted connection.

**Portable workspace**:
The directory that travels with NYRA and holds user settings and deliberately saved artifacts.

**Diagnostic report**:
An operator-requested export of a diagnostic run and its findings, limited to the information selected for sharing.
