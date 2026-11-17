mod tcp;
mod tools;
pub mod tui;

use reqwest::Url;
use serde::Serialize;
use std::{
    fmt,
    future::Future,
    net::IpAddr,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub struct ToolMetadata {
    pub id: &'static str,
    pub version: u8,
    pub name: &'static str,
    pub description: &'static str,
    pub category: ToolCategory,
    pub keywords: &'static [&'static str],
    pub aliases: &'static [&'static str],
    pub sample_queries: &'static [&'static str],
    pub sample_inputs: &'static [&'static str],
    pub input_fields: &'static [InputField],
    pub platforms: &'static [OperatingSystem],
    pub platform_status: PlatformStatus,
    pub backend: ExecutionBackend,
    pub required_privileges: &'static [&'static str],
    pub dependencies: &'static [&'static str],
    pub risk: RiskLevel,
    pub max_timeout_seconds: Option<u64>,
    pub max_targets: u8,
    pub max_parallel_operations: u8,
    pub max_output_items: Option<usize>,
    pub cancellable: bool,
    pub smoke_tested_platforms: &'static [OperatingSystem],
    pub output_fields: &'static [OutputField],
    pub output_schema: OutputSchema,
    pub status_explanation: &'static str,
    pub help: &'static str,
    pub suggested_follow_ups: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCategory {
    Network,
    System,
    Security,
    File,
}

impl fmt::Display for ToolCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Network => "Network",
            Self::System => "System",
            Self::Security => "Security",
            Self::File => "File",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiskLevel {
    R0,
    R1,
    R2,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::R0 => "R0",
            Self::R1 => "R1",
            Self::R2 => "R2",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatingSystem {
    Windows,
    Linux,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformStatus {
    SmokeTested,
    NeedsSmokeTest,
}

impl fmt::Display for PlatformStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SmokeTested => "smoke tested",
            Self::NeedsSmokeTest => "needs smoke test",
        })
    }
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Windows => "Windows",
            Self::Linux => "Linux",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionBackend {
    Native,
    OperatingSystem,
}

impl fmt::Display for ExecutionBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Native => "native",
            Self::OperatingSystem => "operating_system",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Host,
    IpAddress,
    Port,
    Url,
    Path,
    TimeoutSeconds,
    RecordTypes,
    Count,
}

#[derive(Clone, Copy)]
pub struct InputField {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: InputKind,
    pub required: bool,
    pub default: Option<&'static str>,
    pub minimum: Option<u64>,
    pub maximum: Option<u64>,
}

#[derive(Clone, Copy)]
pub struct OutputField {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputSchema {
    FindingOnly,
    DnsData,
    HttpData,
    TlsData,
    InterfaceData,
    PingData,
    SystemInfoData,
    CpuData,
    MemoryData,
    DiskData,
    Sha256Data,
}

impl fmt::Display for OutputSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FindingOnly => "finding_only",
            Self::DnsData => "DnsData",
            Self::HttpData => "HttpData",
            Self::TlsData => "TlsData",
            Self::InterfaceData => "InterfaceData",
            Self::PingData => "PingData",
            Self::SystemInfoData => "SystemInfoData",
            Self::CpuData => "CpuData",
            Self::MemoryData => "MemoryData",
            Self::DiskData => "DiskData",
            Self::Sha256Data => "Sha256Data",
        })
    }
}

const TCP_INPUT_FIELDS: &[InputField] = &[
    InputField {
        name: "host",
        label: "Host or IP address",
        kind: InputKind::Host,
        required: true,
        default: None,
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "port",
        label: "TCP port",
        kind: InputKind::Port,
        required: true,
        default: None,
        minimum: Some(1),
        maximum: Some(65_535),
    },
    InputField {
        name: "timeout",
        label: "Total timeout (seconds)",
        kind: InputKind::TimeoutSeconds,
        required: true,
        default: Some("5"),
        minimum: Some(1),
        maximum: Some(60),
    },
];

const TCP_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "finding",
        description: "open, refused, timed out, unreachable, or inconclusive",
    },
    OutputField {
        name: "target",
        description: "the single host and port checked",
    },
    OutputField {
        name: "duration_ms",
        description: "elapsed time in milliseconds",
    },
];

const TCP_ALIASES: &[&str] = &[
    "check port",
    "test port",
    "port connectivity",
    "telnet",
    "เช็คพอร์ต",
    "เช็คพอร์ต 443",
];
const TCP_KEYWORDS: &[&str] = &["tcp", "port", "connection", "network"];
const TCP_SAMPLE_QUERIES: &[&str] = &["check port", "can I reach port 443", "test TCP connection"];
const WINDOWS_LINUX_PLATFORMS: &[OperatingSystem] =
    &[OperatingSystem::Windows, OperatingSystem::Linux];

const TCP: ToolMetadata = ToolMetadata {
    id: "network.tcp",
    version: 1,
    name: "TCP Port Check",
    description: "Check whether one host accepts a TCP connection",
    category: ToolCategory::Network,
    keywords: TCP_KEYWORDS,
    aliases: TCP_ALIASES,
    sample_queries: TCP_SAMPLE_QUERIES,
    sample_inputs: &["example.com", "443", "5"],
    input_fields: TCP_INPUT_FIELDS,
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &[],
    risk: RiskLevel::R1,
    max_timeout_seconds: Some(60),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: None,
    cancellable: true,
    smoke_tested_platforms: &[OperatingSystem::Windows],
    output_fields: TCP_OUTPUT_FIELDS,
    output_schema: OutputSchema::FindingOnly,
    status_explanation: "OPEN = accepted; other findings report refusal, timeout, unreachable, or uncertainty.",
    help: "Use `nyra net tcp <host> <port> --timeout 5` to check one TCP endpoint.",
    suggested_follow_ups: &["network.dns", "security.tls", "web.http"],
};

const SYSTEM_INFO_OUTPUTS: &[OutputField] = &[
    OutputField {
        name: "os",
        description: "operating system identifier",
    },
    OutputField {
        name: "architecture",
        description: "processor architecture reported by the operating system",
    },
    OutputField {
        name: "hostname",
        description: "local host name when available",
    },
    OutputField {
        name: "uptime_seconds",
        description: "host uptime in seconds when available",
    },
    OutputField {
        name: "os_version",
        description: "operating system version when available",
    },
    OutputField {
        name: "kernel_version",
        description: "kernel version when available",
    },
];

const SYSTEM_INFO: ToolMetadata = ToolMetadata {
    id: "system.info",
    version: 1,
    name: "System Information",
    description: "Identify the local operating system and host",
    category: ToolCategory::System,
    keywords: &["system", "os", "host", "machine"],
    aliases: &["system info", "host info", "what machine is this"],
    sample_queries: &["identify this host", "show operating system"],
    sample_inputs: &[],
    input_fields: &[],
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &[],
    risk: RiskLevel::R0,
    max_timeout_seconds: None,
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: None,
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: SYSTEM_INFO_OUTPUTS,
    output_schema: OutputSchema::SystemInfoData,
    status_explanation: "AVAILABLE = operating system details read; unavailable fields are null.",
    help: "Use `nyra sys info` to inspect local host and operating system details.",
    suggested_follow_ups: &["system.cpu", "system.memory", "system.disk"],
};

const CPU_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "brand",
        description: "processor brand string when available",
    },
    OutputField {
        name: "logical_cores",
        description: "number of logical processors",
    },
    OutputField {
        name: "physical_cores",
        description: "physical core count when available",
    },
    OutputField {
        name: "average_usage_percent",
        description: "short sampled CPU utilization percentage",
    },
    OutputField {
        name: "frequency_mhz",
        description: "reported frequency of the first logical processor",
    },
    OutputField {
        name: "per_core_usage_percent",
        description: "sampled utilization for up to 1024 logical processors",
    },
];

const CPU_INFO: ToolMetadata = ToolMetadata {
    id: "system.cpu",
    version: 1,
    name: "CPU Information",
    description: "Show processor details and a short utilization sample",
    category: ToolCategory::System,
    keywords: &["cpu", "processor", "load", "utilization"],
    aliases: &["cpu usage", "processor load", "cpu information"],
    sample_queries: &["how busy is the cpu", "show processor count"],
    sample_inputs: &[],
    input_fields: &[],
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &[],
    risk: RiskLevel::R0,
    max_timeout_seconds: Some(5),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(1024),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: CPU_OUTPUT_FIELDS,
    output_schema: OutputSchema::CpuData,
    status_explanation: "MEASURED = CPU details and a short utilization sample collected.",
    help: "CPU utilization is sampled briefly; the returned value is not an instantaneous reading.",
    suggested_follow_ups: &["system.memory", "system.info"],
};

const MEMORY_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "total_bytes",
        description: "installed memory in bytes",
    },
    OutputField {
        name: "used_bytes",
        description: "total minus available memory",
    },
    OutputField {
        name: "available_bytes",
        description: "memory reported available by the operating system",
    },
];

const MEMORY: ToolMetadata = ToolMetadata {
    id: "system.memory",
    version: 1,
    name: "Memory Usage",
    description: "Measure total, used, and available memory",
    category: ToolCategory::System,
    keywords: &["memory", "ram", "available", "pressure"],
    aliases: &["memory usage", "ram usage", "free memory"],
    sample_queries: &["how much memory is free", "check ram usage"],
    sample_inputs: &[],
    input_fields: &[],
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &[],
    risk: RiskLevel::R0,
    max_timeout_seconds: None,
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: None,
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: MEMORY_OUTPUT_FIELDS,
    output_schema: OutputSchema::MemoryData,
    status_explanation: "MEASURED = total, used, and available memory values read.",
    help: "Memory values are reported in bytes using the operating system's available-memory measure.",
    suggested_follow_ups: &["system.cpu", "system.disk"],
};

const DISK_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "name",
        description: "volume or device name",
    },
    OutputField {
        name: "mount_point",
        description: "volume or mount point",
    },
    OutputField {
        name: "total_bytes",
        description: "volume size in bytes",
    },
    OutputField {
        name: "available_bytes",
        description: "space available to the current user",
    },
    OutputField {
        name: "file_system",
        description: "reported filesystem type",
    },
];

const DISK: ToolMetadata = ToolMetadata {
    id: "system.disk",
    version: 1,
    name: "Disk Usage",
    description: "List local volumes with total and available space",
    category: ToolCategory::System,
    keywords: &["disk", "volume", "storage", "space", "filesystem"],
    aliases: &["disk space", "free disk", "disk usage"],
    sample_queries: &["which disk is full", "check disk space"],
    sample_inputs: &[],
    input_fields: &[],
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &[],
    risk: RiskLevel::R0,
    max_timeout_seconds: None,
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(256),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: DISK_OUTPUT_FIELDS,
    output_schema: OutputSchema::DiskData,
    status_explanation: "MEASURED = volume totals and available space collected.",
    help: "Use `nyra sys disk` to list local mounted volumes; network mounts are not probed.",
    suggested_follow_ups: &["file.sha256"],
};

const SHA256_INPUT_FIELDS: &[InputField] = &[
    InputField {
        name: "path",
        label: "File path",
        kind: InputKind::Path,
        required: true,
        default: None,
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "timeout",
        label: "Total timeout (seconds)",
        kind: InputKind::TimeoutSeconds,
        required: true,
        default: Some("30"),
        minimum: Some(1),
        maximum: Some(60),
    },
];

const SHA256_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "sha256",
        description: "lowercase hexadecimal SHA-256 digest",
    },
    OutputField {
        name: "size_bytes",
        description: "number of bytes hashed",
    },
    OutputField {
        name: "path",
        description: "path of the input file",
    },
];

const SHA256: ToolMetadata = ToolMetadata {
    id: "file.sha256",
    version: 1,
    name: "SHA-256 File Hash",
    description: "Compute a file's SHA-256 digest with bounded memory use",
    category: ToolCategory::File,
    keywords: &["sha256", "hash", "checksum", "file integrity"],
    aliases: &["calculate file hash", "check file checksum", "sha-256"],
    sample_queries: &["calculate a file hash", "verify a file checksum"],
    sample_inputs: &["./artifact.bin", "10"],
    input_fields: SHA256_INPUT_FIELDS,
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &[],
    risk: RiskLevel::R0,
    max_timeout_seconds: Some(60),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: None,
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: SHA256_OUTPUT_FIELDS,
    output_schema: OutputSchema::Sha256Data,
    status_explanation: "COMPUTED = regular file read completely and hashed.",
    help: "Reads one file in bounded chunks. The path must refer to a regular file.",
    suggested_follow_ups: &[],
};

const DNS_INPUT_FIELDS: &[InputField] = &[
    InputField {
        name: "hostname",
        label: "Hostname",
        kind: InputKind::Host,
        required: true,
        default: None,
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "record_types",
        label: "Record types (comma separated)",
        kind: InputKind::RecordTypes,
        required: true,
        default: Some("A,AAAA"),
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "timeout",
        label: "Total timeout (seconds)",
        kind: InputKind::TimeoutSeconds,
        required: true,
        default: Some("5"),
        minimum: Some(1),
        maximum: Some(60),
    },
];
const DNS_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "hostname",
        description: "queried DNS name",
    },
    OutputField {
        name: "records",
        description: "bounded answers grouped by record type",
    },
];
const DNS: ToolMetadata = ToolMetadata {
    id: "network.dns",
    version: 1,
    name: "DNS Lookup",
    description: "Query selected DNS record types using the system resolver configuration",
    category: ToolCategory::Network,
    keywords: &["dns", "resolve", "domain", "name", "address"],
    aliases: &[
        "dns lookup",
        "resolve hostname",
        "check dns",
        "name resolution",
    ],
    sample_queries: &["resolve example.com", "check A and AAAA records"],
    sample_inputs: &["example.com", "A,AAAA", "5"],
    input_fields: DNS_INPUT_FIELDS,
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &["system DNS configuration"],
    risk: RiskLevel::R1,
    max_timeout_seconds: Some(60),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(100),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: DNS_OUTPUT_FIELDS,
    output_schema: OutputSchema::DnsData,
    status_explanation: "AVAILABLE = at least one answer; INCONCLUSIVE = no selected records.",
    help: "Supports A, AAAA, CNAME, MX, and TXT records; default queries are A and AAAA.",
    suggested_follow_ups: &["network.tcp", "network.reverse_dns"],
};

const REVERSE_DNS_INPUT_FIELDS: &[InputField] = &[
    InputField {
        name: "address",
        label: "IP address",
        kind: InputKind::IpAddress,
        required: true,
        default: None,
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "timeout",
        label: "Total timeout (seconds)",
        kind: InputKind::TimeoutSeconds,
        required: true,
        default: Some("5"),
        minimum: Some(1),
        maximum: Some(60),
    },
];
const REVERSE_DNS_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "hostname",
        description: "queried IP address",
    },
    OutputField {
        name: "records",
        description: "bounded PTR answers",
    },
];
const REVERSE_DNS: ToolMetadata = ToolMetadata {
    id: "network.reverse_dns",
    version: 1,
    name: "Reverse DNS Lookup",
    description: "Look up PTR records for one IP address using the system resolver",
    category: ToolCategory::Network,
    keywords: &["reverse dns", "ptr", "hostname", "address"],
    aliases: &["reverse lookup", "ip to hostname", "ptr lookup"],
    sample_queries: &["find the hostname for this IP", "reverse DNS lookup"],
    sample_inputs: &["192.0.2.10", "5"],
    input_fields: REVERSE_DNS_INPUT_FIELDS,
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &["system DNS configuration"],
    risk: RiskLevel::R1,
    max_timeout_seconds: Some(60),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(20),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: REVERSE_DNS_OUTPUT_FIELDS,
    output_schema: OutputSchema::DnsData,
    status_explanation: "AVAILABLE = PTR answer returned; INCONCLUSIVE = no PTR record.",
    help: "Queries one PTR name. A missing PTR answer is reported as an inconclusive diagnostic result.",
    suggested_follow_ups: &["network.dns", "network.tcp"],
};

const HTTP_INPUT_FIELDS: &[InputField] = &[
    InputField {
        name: "url",
        label: "HTTP or HTTPS URL",
        kind: InputKind::Url,
        required: true,
        default: None,
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "timeout",
        label: "Total timeout (seconds)",
        kind: InputKind::TimeoutSeconds,
        required: true,
        default: Some("10"),
        minimum: Some(1),
        maximum: Some(60),
    },
];
const HTTP_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "url",
        description: "URL with query and fragment removed",
    },
    OutputField {
        name: "status_code",
        description: "HTTP response status code",
    },
    OutputField {
        name: "elapsed_ms",
        description: "time to receive response headers",
    },
    OutputField {
        name: "headers",
        description: "small allowlist of response headers",
    },
    OutputField {
        name: "redirect_location",
        description: "redacted destination from a redirect response, when available",
    },
];
const HTTP: ToolMetadata = ToolMetadata {
    id: "web.http",
    version: 1,
    name: "HTTP Health Check",
    description: "Request one URL and report the status and selected response headers",
    category: ToolCategory::Network,
    keywords: &["http", "https", "web", "website", "status", "health"],
    aliases: &[
        "http check",
        "website health",
        "web health",
        "check website",
    ],
    sample_queries: &["check https://example.com", "is this website responding"],
    sample_inputs: &["https://example.com/health", "5"],
    input_fields: HTTP_INPUT_FIELDS,
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &["TLS root certificates for HTTPS"],
    risk: RiskLevel::R1,
    max_timeout_seconds: Some(60),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(5),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: HTTP_OUTPUT_FIELDS,
    output_schema: OutputSchema::HttpData,
    status_explanation: "HEALTHY = HTTP 2xx; UNHEALTHY = another response status.",
    help: "Does not follow redirects or read response bodies. A redacted redirect destination is reported when the server provides one.",
    suggested_follow_ups: &["network.dns", "network.tcp", "security.tls"],
};

const TLS_INPUT_FIELDS: &[InputField] = &[
    InputField {
        name: "host",
        label: "Hostname, IP address, or HTTPS URL",
        kind: InputKind::Host,
        required: true,
        default: None,
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "port",
        label: "TLS port",
        kind: InputKind::Port,
        required: true,
        default: Some("443"),
        minimum: Some(1),
        maximum: Some(65_535),
    },
    InputField {
        name: "timeout",
        label: "Total timeout (seconds)",
        kind: InputKind::TimeoutSeconds,
        required: true,
        default: Some("10"),
        minimum: Some(1),
        maximum: Some(60),
    },
];
const TLS_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "subject",
        description: "leaf certificate subject",
    },
    OutputField {
        name: "issuer",
        description: "leaf certificate issuer",
    },
    OutputField {
        name: "subject_alt_names",
        description: "DNS and IP subject alternative names",
    },
    OutputField {
        name: "not_before_unix",
        description: "certificate start time as Unix seconds",
    },
    OutputField {
        name: "not_after_unix",
        description: "certificate expiry as Unix seconds",
    },
    OutputField {
        name: "trust_validation",
        description: "system TLS validation outcome",
    },
];
const TLS: ToolMetadata = ToolMetadata {
    id: "security.tls",
    version: 1,
    name: "TLS Certificate Inspect",
    description: "Inspect a TLS endpoint's leaf certificate and system validation status",
    category: ToolCategory::Security,
    keywords: &["tls", "ssl", "certificate", "expiry", "https"],
    aliases: &[
        "certificate inspect",
        "check certificate",
        "certificate expiry",
        "ssl check",
    ],
    sample_queries: &[
        "check certificate for example.com",
        "when does this certificate expire",
    ],
    sample_inputs: &["example.com", "443", "5"],
    input_fields: TLS_INPUT_FIELDS,
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::Native,
    required_privileges: &[],
    dependencies: &["TLS root certificates"],
    risk: RiskLevel::R1,
    max_timeout_seconds: Some(60),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(64),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: TLS_OUTPUT_FIELDS,
    output_schema: OutputSchema::TlsData,
    status_explanation: "AVAILABLE = system trust passed; UNHEALTHY = trust failed, certificate inspected.",
    help: "Accepts a hostname, IP address, or credential-free HTTPS URL. Completes an inspection-only TLS handshake, records normal system validation, and sends no application data.",
    suggested_follow_ups: &["network.tcp", "web.http"],
};

const INTERFACES_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "interfaces",
        description: "up to 64 local network addresses and netmasks",
    },
    OutputField {
        name: "name",
        description: "local interface name",
    },
    OutputField {
        name: "address",
        description: "local IPv4 or IPv6 address",
    },
    OutputField {
        name: "netmask",
        description: "interface address netmask",
    },
    OutputField {
        name: "loopback",
        description: "whether the address is loopback",
    },
];
const INTERFACES: ToolMetadata = ToolMetadata {
    id: "network.interfaces",
    version: 1,
    name: "Local Network Interfaces",
    description: "List local network interface addresses and netmasks",
    category: ToolCategory::Network,
    keywords: &["network", "interface", "ip", "address", "adapter"],
    aliases: &[
        "network interfaces",
        "show ip addresses",
        "local ip",
        "ip config",
    ],
    sample_queries: &[
        "show local network interfaces",
        "what is this machine's IP address",
    ],
    sample_inputs: &[],
    input_fields: &[],
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::OperatingSystem,
    required_privileges: &[],
    dependencies: &[],
    risk: RiskLevel::R0,
    max_timeout_seconds: None,
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(64),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: INTERFACES_OUTPUT_FIELDS,
    output_schema: OutputSchema::InterfaceData,
    status_explanation: "AVAILABLE = local interface addresses and netmasks read.",
    help: "Reports addresses and netmasks. Default gateways and DNS servers are not included by this implementation.",
    suggested_follow_ups: &["network.dns", "network.tcp"],
};

const PING_INPUT_FIELDS: &[InputField] = &[
    InputField {
        name: "host",
        label: "Hostname or IP address",
        kind: InputKind::Host,
        required: true,
        default: None,
        minimum: None,
        maximum: None,
    },
    InputField {
        name: "count",
        label: "Echo requests",
        kind: InputKind::Count,
        required: true,
        default: Some("4"),
        minimum: Some(1),
        maximum: Some(10),
    },
    InputField {
        name: "timeout",
        label: "Total timeout (seconds)",
        kind: InputKind::TimeoutSeconds,
        required: true,
        default: Some("5"),
        minimum: Some(1),
        maximum: Some(60),
    },
];
const PING_OUTPUT_FIELDS: &[OutputField] = &[
    OutputField {
        name: "host",
        description: "single validated host or IP address",
    },
    OutputField {
        name: "count",
        description: "configured bounded echo request count",
    },
    OutputField {
        name: "reachable",
        description: "whether the system ping process received a reply",
    },
    OutputField {
        name: "latency_ms",
        description: "first reported round trip time when available",
    },
    OutputField {
        name: "note",
        description: "ICMP filtering and utility caveat",
    },
];
const PING: ToolMetadata = ToolMetadata {
    id: "network.ping",
    version: 1,
    name: "Ping",
    description: "Send a bounded number of ICMP echo requests using the operating system utility",
    category: ToolCategory::Network,
    keywords: &["ping", "icmp", "latency", "reachability"],
    aliases: &[
        "ping host",
        "check host reachable",
        "icmp check",
        "test latency",
    ],
    sample_queries: &["ping example.com", "check if this host is reachable"],
    sample_inputs: &["example.com", "4", "5"],
    input_fields: PING_INPUT_FIELDS,
    platforms: WINDOWS_LINUX_PLATFORMS,
    platform_status: PlatformStatus::NeedsSmokeTest,
    backend: ExecutionBackend::OperatingSystem,
    required_privileges: &[],
    dependencies: &["system ping utility on PATH"],
    risk: RiskLevel::R1,
    max_timeout_seconds: Some(60),
    max_targets: 1,
    max_parallel_operations: 1,
    max_output_items: Some(1),
    cancellable: true,
    smoke_tested_platforms: &[],
    output_fields: PING_OUTPUT_FIELDS,
    output_schema: OutputSchema::PingData,
    status_explanation: "AVAILABLE = echo reply received; UNREACHABLE = no usable reply.",
    help: "The OS ping utility is required. Firewalls may block ICMP even when other services are reachable.",
    suggested_follow_ups: &["network.tcp", "network.dns"],
};

pub const TOOLS: &[ToolMetadata] = &[
    TCP,
    DNS,
    REVERSE_DNS,
    HTTP,
    TLS,
    INTERFACES,
    PING,
    SYSTEM_INFO,
    CPU_INFO,
    MEMORY,
    DISK,
    SHA256,
];

pub fn find_tool(id: &str) -> Option<&'static ToolMetadata> {
    TOOLS.iter().find(|tool| tool.id == id)
}

pub use tcp::TcpTarget;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Finding {
    Open,
    Refused,
    TimedOut,
    Unreachable,
    Inconclusive,
    Available,
    Measured,
    Computed,
    Healthy,
    Unhealthy,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Open => "OPEN",
            Self::Refused => "REFUSED",
            Self::TimedOut => "TIMED OUT",
            Self::Unreachable => "UNREACHABLE",
            Self::Inconclusive => "INCONCLUSIVE",
            Self::Available => "AVAILABLE",
            Self::Measured => "MEASURED",
            Self::Computed => "COMPUTED",
            Self::Healthy => "HEALTHY",
            Self::Unhealthy => "UNHEALTHY",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunError {
    ResolutionFailed,
    SocketFailed,
    DeadlineExceeded,
    Cancelled,
    SignalUnavailable,
    SystemInfoUnavailable,
    FileOpenFailed,
    FileReadFailed,
    ResolverUnavailable,
    InvalidInput,
    HttpClientFailed,
    NetworkRequestFailed,
    TlsCertificateUnavailable,
    TlsCertificateInvalid,
    TlsVerifierUnavailable,
    TlsHandshakeFailed,
    InterfaceQueryFailed,
    DependencyUnavailable,
    UnsupportedPlatform,
    PingFailed,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ResolutionFailed => "Could not resolve the target host",
            Self::SocketFailed => "The operating system could not complete the socket check",
            Self::DeadlineExceeded => {
                "The run reached its deadline before it could report a finding"
            }
            Self::Cancelled => "The check was cancelled",
            Self::SignalUnavailable => "Could not register the interrupt handler",
            Self::SystemInfoUnavailable => {
                "The operating system did not provide system information"
            }
            Self::FileOpenFailed => "Could not open the requested file",
            Self::FileReadFailed => "Could not read the requested file",
            Self::ResolverUnavailable => "Could not load the system DNS resolver configuration",
            Self::InvalidInput => "The diagnostic input is invalid",
            Self::HttpClientFailed => "Could not initialize the HTTP client",
            Self::NetworkRequestFailed => "The network request could not be completed",
            Self::TlsCertificateUnavailable => {
                "The TLS peer did not provide an inspectable certificate"
            }
            Self::TlsCertificateInvalid => "Could not parse the TLS peer certificate",
            Self::TlsVerifierUnavailable => "Could not initialize the system TLS verifier",
            Self::TlsHandshakeFailed => "The TLS handshake could not be completed",
            Self::InterfaceQueryFailed => "Could not read local network interface information",
            Self::UnsupportedPlatform => {
                "This diagnostic is not available on the current operating system"
            }
            Self::DependencyUnavailable => {
                "REQUIRES SETUP: NYRA ping needs the Windows or Linux system `ping` utility on PATH"
            }
            Self::PingFailed => "The system `ping` utility reported an execution error",
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ExecutionStatus {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Serialize)]
pub struct RunResult {
    pub schema_version: u8,
    pub tool_id: &'static str,
    pub target: String,
    execution_status: ExecutionStatus,
    pub finding: Option<Finding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<OutputData>,
    pub duration_ms: u128,
    pub error: Option<RunError>,
}

impl RunResult {
    pub fn execution_status_label(&self) -> &'static str {
        match &self.execution_status {
            ExecutionStatus::Completed => "completed",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::Cancelled => "cancelled",
        }
    }

    pub fn exit_code(&self) -> u8 {
        match (self.finding, self.error) {
            (
                Some(
                    Finding::Open
                    | Finding::Available
                    | Finding::Measured
                    | Finding::Computed
                    | Finding::Healthy,
                ),
                None,
            ) => 0,
            (Some(_), None) => 1,
            (_, Some(RunError::DeadlineExceeded)) => 4,
            (_, Some(RunError::Cancelled)) => 130,
            _ => 3,
        }
    }
}

pub enum ToolInput {
    Tcp(TcpTarget),
    Dns(DnsTarget),
    ReverseDns(ReverseDnsTarget),
    Http(HttpTarget),
    Tls(TlsTarget),
    Interfaces,
    Ping(PingTarget),
    SystemInfo,
    Cpu,
    Memory,
    Disk,
    Sha256(Sha256Target),
}

impl ToolInput {
    fn tool_id(&self) -> &'static str {
        match self {
            Self::Tcp(_) => "network.tcp",
            Self::Dns(_) => "network.dns",
            Self::ReverseDns(_) => "network.reverse_dns",
            Self::Http(_) => "web.http",
            Self::Tls(_) => "security.tls",
            Self::Interfaces => "network.interfaces",
            Self::Ping(_) => "network.ping",
            Self::SystemInfo => "system.info",
            Self::Cpu => "system.cpu",
            Self::Memory => "system.memory",
            Self::Disk => "system.disk",
            Self::Sha256(_) => "file.sha256",
        }
    }

    fn target(&self) -> String {
        match self {
            Self::Tcp(target) => target.to_string(),
            Self::Dns(target) => target.hostname.clone(),
            Self::ReverseDns(target) => target.address.to_string(),
            Self::Http(target) => tools::network::safe_url(&target.url),
            Self::Tls(target) if target.host.contains(':') => {
                format!("[{}]:{}", target.host, target.port)
            }
            Self::Tls(target) => format!("{}:{}", target.host, target.port),
            Self::Interfaces => "local".into(),
            Self::Ping(target) => target.host.clone(),
            Self::SystemInfo => "local".into(),
            Self::Cpu | Self::Memory | Self::Disk => "local".into(),
            Self::Sha256(target) => target.path.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum OutputData {
    Dns(DnsData),
    Http(HttpData),
    Tls(TlsData),
    Interfaces(InterfaceData),
    Ping(PingData),
    SystemInfo(SystemInfoData),
    Cpu(CpuData),
    Memory(MemoryData),
    Disk(DiskData),
    Sha256(Sha256Data),
}

#[derive(Serialize)]
pub struct DnsData {
    pub hostname: String,
    pub records: Vec<DnsRecord>,
}

#[derive(Serialize)]
pub struct DnsRecord {
    pub record_type: String,
    pub answers: Vec<String>,
}

#[derive(Serialize)]
pub struct HttpData {
    pub url: String,
    pub status_code: u16,
    pub elapsed_ms: u128,
    pub headers: std::collections::BTreeMap<String, String>,
    pub redirect_location: Option<String>,
}

#[derive(Serialize)]
pub struct TlsData {
    pub host: String,
    pub port: u16,
    pub subject: String,
    pub issuer: String,
    pub subject_alt_names: Vec<String>,
    pub not_before_unix: i64,
    pub not_after_unix: i64,
    pub elapsed_ms: u128,
    pub trust_validation: String,
}

#[derive(Serialize)]
pub struct InterfaceData {
    pub interfaces: Vec<InterfaceEntry>,
}

#[derive(Serialize)]
pub struct InterfaceEntry {
    pub name: String,
    pub address: String,
    pub netmask: String,
    pub loopback: bool,
}

#[derive(Serialize)]
pub struct PingData {
    pub host: String,
    pub count: u8,
    pub reachable: bool,
    pub latency_ms: Option<f64>,
    pub note: String,
}

#[derive(Serialize)]
pub struct SystemInfoData {
    pub os: String,
    pub architecture: String,
    pub hostname: Option<String>,
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Serialize)]
pub struct CpuData {
    pub brand: Option<String>,
    pub logical_cores: usize,
    pub physical_cores: Option<usize>,
    pub frequency_mhz: Option<u64>,
    pub average_usage_percent: Option<f32>,
    pub per_core_usage_percent: Vec<f32>,
}

#[derive(Serialize)]
pub struct MemoryData {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Serialize)]
pub struct DiskData {
    pub volumes: Vec<DiskVolume>,
}

#[derive(Serialize)]
pub struct DiskVolume {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Serialize)]
pub struct Sha256Data {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub struct Sha256Target {
    path: PathBuf,
    timeout_seconds: u64,
}

impl Sha256Target {
    pub fn new(path: String, timeout_seconds: u64) -> Result<Self, &'static str> {
        if path.trim().is_empty() || path.contains('\0') {
            return Err("File path must not be empty or contain a null character");
        }
        validate_numeric_input("file.sha256", "timeout", timeout_seconds)?;
        Ok(Self {
            path: PathBuf::from(path),
            timeout_seconds,
        })
    }
}

pub struct DnsTarget {
    hostname: String,
    record_types: Vec<String>,
    timeout_seconds: u64,
}

impl DnsTarget {
    pub fn new(
        hostname: String,
        record_types: String,
        timeout_seconds: u64,
    ) -> Result<Self, &'static str> {
        if hostname.parse::<IpAddr>().is_ok() || !tcp::valid_hostname(&hostname) {
            return Err("Hostname must be a valid DNS name, not an IP address");
        }
        let mut parsed = Vec::new();
        for kind in record_types
            .split(',')
            .map(str::trim)
            .filter(|kind| !kind.is_empty())
        {
            let kind = kind.to_ascii_uppercase();
            if !["A", "AAAA", "CNAME", "MX", "TXT"].contains(&kind.as_str()) {
                return Err("Record types must be A, AAAA, CNAME, MX, or TXT");
            }
            if !parsed.contains(&kind) {
                parsed.push(kind);
            }
        }
        if parsed.is_empty() || parsed.len() > 5 {
            return Err("Choose between 1 and 5 DNS record types");
        }
        validate_numeric_input("network.dns", "timeout", timeout_seconds)?;
        Ok(Self {
            hostname,
            record_types: parsed,
            timeout_seconds,
        })
    }
}

pub struct ReverseDnsTarget {
    address: IpAddr,
    timeout_seconds: u64,
}

impl ReverseDnsTarget {
    pub fn new(address: String, timeout_seconds: u64) -> Result<Self, &'static str> {
        let address = address
            .parse()
            .map_err(|_| "Address must be a valid IPv4 or IPv6 address")?;
        validate_numeric_input("network.reverse_dns", "timeout", timeout_seconds)?;
        Ok(Self {
            address,
            timeout_seconds,
        })
    }
}

pub struct HttpTarget {
    url: Url,
    timeout_seconds: u64,
}

impl HttpTarget {
    pub fn new(value: String, timeout_seconds: u64) -> Result<Self, &'static str> {
        if value.len() > 2048 || value.contains('\0') {
            return Err("URL must be at most 2048 bytes and contain no null characters");
        }
        let url = Url::parse(&value).map_err(|_| "URL must be a valid HTTP or HTTPS URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("URL must use HTTP or HTTPS and include a hostname");
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("URL must not include username or password credentials");
        }
        validate_numeric_input("web.http", "timeout", timeout_seconds)?;
        Ok(Self {
            url,
            timeout_seconds,
        })
    }
}

pub struct TlsTarget {
    host: String,
    port: u16,
    timeout_seconds: u64,
}

impl TlsTarget {
    pub fn new(host: String, port: u16, timeout_seconds: u64) -> Result<Self, &'static str> {
        let host = if host.contains("://") {
            let url = Url::parse(&host)
                .map_err(|_| "Host must be an IP address, DNS hostname, or HTTPS URL")?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(
                    "Host URL must use HTTPS, include a hostname, and contain no credentials",
                );
            }
            if url.port().is_some_and(|url_port| url_port != port) {
                return Err("Port field must match the HTTPS URL port");
            }
            url.host_str()
                .ok_or("HTTPS URL must include a hostname")?
                .to_owned()
        } else {
            host
        };
        if host.parse::<IpAddr>().is_err() && !tcp::valid_hostname(&host) {
            return Err("Host must be an IP address, DNS hostname, or HTTPS URL");
        }
        validate_numeric_input("security.tls", "port", port.into())?;
        validate_numeric_input("security.tls", "timeout", timeout_seconds)?;
        Ok(Self {
            host,
            port,
            timeout_seconds,
        })
    }
}

pub struct PingTarget {
    host: String,
    count: u8,
    timeout_seconds: u64,
}

impl PingTarget {
    pub fn new(host: String, count: u8, timeout_seconds: u64) -> Result<Self, &'static str> {
        if host.parse::<IpAddr>().is_err() && !tcp::valid_hostname(&host) {
            return Err("Host must be an IPv4 address, IPv6 address, or DNS hostname");
        }
        validate_numeric_input("network.ping", "count", count.into())?;
        validate_numeric_input("network.ping", "timeout", timeout_seconds)?;
        Ok(Self {
            host,
            count,
            timeout_seconds,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InputError {
    pub field_index: usize,
    pub message: &'static str,
}

pub(crate) fn validate_numeric_input(
    tool_id: &str,
    field_name: &str,
    value: u64,
) -> Result<(), &'static str> {
    let tool = find_tool(tool_id).ok_or("Unknown diagnostic tool")?;
    let field = tool
        .input_fields
        .iter()
        .find(|field| field.name == field_name)
        .ok_or("Tool input metadata is incomplete")?;
    let minimum = field.minimum.unwrap_or(0);
    let mut maximum = field.maximum.unwrap_or(u64::MAX);
    if matches!(field.kind, InputKind::TimeoutSeconds)
        && let Some(tool_maximum) = tool.max_timeout_seconds
    {
        maximum = maximum.min(tool_maximum);
    }
    if (minimum..=maximum).contains(&value) {
        return Ok(());
    }
    Err(match field.kind {
        InputKind::Port => "Port is outside the configured range",
        InputKind::TimeoutSeconds => "Timeout is outside the configured range",
        InputKind::Count => "Count is outside the configured range",
        _ => "Value is outside the configured limits",
    })
}

fn validate_metadata_fields(tool: &ToolMetadata, fields: &[String]) -> Result<(), InputError> {
    if fields.len() != tool.input_fields.len() {
        return Err(InputError {
            field_index: fields.len().min(tool.input_fields.len().saturating_sub(1)),
            message: if tool.input_fields.is_empty() {
                "This tool does not accept input fields"
            } else {
                "Provide a value for each tool input"
            },
        });
    }

    for (field_index, (field, value)) in tool.input_fields.iter().zip(fields).enumerate() {
        if field.required && value.trim().is_empty() {
            return Err(InputError {
                field_index,
                message: "This field is required",
            });
        }
        match field.kind {
            InputKind::Port | InputKind::TimeoutSeconds | InputKind::Count => {
                let number = value.parse::<u64>().map_err(|_| InputError {
                    field_index,
                    message: match field.kind {
                        InputKind::Port => "Port must be a whole number",
                        InputKind::TimeoutSeconds => "Timeout must be a whole number",
                        InputKind::Count => "Count must be a whole number",
                        _ => "Enter a whole number",
                    },
                })?;
                validate_numeric_input(tool.id, field.name, number).map_err(|message| {
                    InputError {
                        field_index,
                        message,
                    }
                })?;
            }
            InputKind::IpAddress => {
                value.parse::<IpAddr>().map_err(|_| InputError {
                    field_index,
                    message: "Address must be a valid IPv4 or IPv6 address",
                })?;
            }
            InputKind::Host | InputKind::Url | InputKind::Path | InputKind::RecordTypes => {}
        }
    }
    Ok(())
}

pub fn parse_tool_input(tool_id: &str, fields: &[String]) -> Result<ToolInput, InputError> {
    let tool = find_tool(tool_id).ok_or(InputError {
        field_index: 0,
        message: "Unknown diagnostic tool",
    })?;
    validate_metadata_fields(tool, fields)?;
    match tool_id {
        "network.interfaces" if fields.is_empty() => Ok(ToolInput::Interfaces),
        "network.interfaces" => Err(InputError {
            field_index: 0,
            message: "Local Network Interfaces does not accept input fields",
        }),
        "network.dns" => {
            if fields.len() != 3 {
                return Err(InputError {
                    field_index: 0,
                    message: "DNS Lookup requires a hostname, record types, and timeout",
                });
            }
            let timeout = fields[2].parse::<u64>().map_err(|_| InputError {
                field_index: 2,
                message: "Timeout must be between 1 and 60 seconds",
            })?;
            DnsTarget::new(fields[0].clone(), fields[1].clone(), timeout)
                .map(ToolInput::Dns)
                .map_err(|message| InputError {
                    field_index: if message.starts_with("Hostname") {
                        0
                    } else if message.starts_with("Record") || message.starts_with("Choose") {
                        1
                    } else {
                        2
                    },
                    message,
                })
        }
        "network.reverse_dns" => {
            if fields.len() != 2 {
                return Err(InputError {
                    field_index: 0,
                    message: "Reverse DNS Lookup requires an IP address and timeout",
                });
            }
            let timeout = fields[1].parse::<u64>().map_err(|_| InputError {
                field_index: 1,
                message: "Timeout must be between 1 and 60 seconds",
            })?;
            ReverseDnsTarget::new(fields[0].clone(), timeout)
                .map(ToolInput::ReverseDns)
                .map_err(|message| InputError {
                    field_index: if message.starts_with("Address") { 0 } else { 1 },
                    message,
                })
        }
        "web.http" => {
            if fields.len() != 2 {
                return Err(InputError {
                    field_index: 0,
                    message: "HTTP Health Check requires a URL and timeout",
                });
            }
            let timeout = fields[1].parse::<u64>().map_err(|_| InputError {
                field_index: 1,
                message: "Timeout must be between 1 and 60 seconds",
            })?;
            HttpTarget::new(fields[0].clone(), timeout)
                .map(ToolInput::Http)
                .map_err(|message| InputError {
                    field_index: if message.starts_with("Timeout") { 1 } else { 0 },
                    message,
                })
        }
        "security.tls" => {
            if fields.len() != 3 {
                return Err(InputError {
                    field_index: 0,
                    message: "TLS Certificate Inspect requires a host, port, and timeout",
                });
            }
            let port = fields[1].parse::<u16>().map_err(|_| InputError {
                field_index: 1,
                message: "Port must be between 1 and 65535",
            })?;
            let timeout = fields[2].parse::<u64>().map_err(|_| InputError {
                field_index: 2,
                message: "Timeout must be between 1 and 60 seconds",
            })?;
            TlsTarget::new(fields[0].clone(), port, timeout)
                .map(ToolInput::Tls)
                .map_err(|message| InputError {
                    field_index: if message.starts_with("Host") {
                        0
                    } else if message.starts_with("Port") {
                        1
                    } else {
                        2
                    },
                    message,
                })
        }
        "network.ping" => {
            if fields.len() != 3 {
                return Err(InputError {
                    field_index: 0,
                    message: "Ping requires a host, count, and timeout",
                });
            }
            let count = fields[1].parse::<u8>().map_err(|_| InputError {
                field_index: 1,
                message: "Ping count must be between 1 and 10",
            })?;
            let timeout = fields[2].parse::<u64>().map_err(|_| InputError {
                field_index: 2,
                message: "Timeout must be between 1 and 60 seconds",
            })?;
            PingTarget::new(fields[0].clone(), count, timeout)
                .map(ToolInput::Ping)
                .map_err(|message| InputError {
                    field_index: if message.starts_with("Host") {
                        0
                    } else if message.starts_with("Ping") {
                        1
                    } else {
                        2
                    },
                    message,
                })
        }
        "system.info" if fields.is_empty() => Ok(ToolInput::SystemInfo),
        "system.info" => Err(InputError {
            field_index: 0,
            message: "System Information does not accept input fields",
        }),
        "system.cpu" if fields.is_empty() => Ok(ToolInput::Cpu),
        "system.memory" if fields.is_empty() => Ok(ToolInput::Memory),
        "system.disk" if fields.is_empty() => Ok(ToolInput::Disk),
        "file.sha256" => {
            if fields.len() != 2 {
                return Err(InputError {
                    field_index: 0,
                    message: "SHA-256 File Hash requires a file path and timeout",
                });
            }
            let timeout = fields[1].parse::<u64>().map_err(|_| InputError {
                field_index: 1,
                message: "Timeout must be between 1 and 60 seconds",
            })?;
            Sha256Target::new(fields[0].clone(), timeout)
                .map(ToolInput::Sha256)
                .map_err(|message| InputError {
                    field_index: if message.starts_with("File") { 0 } else { 1 },
                    message,
                })
        }
        "network.tcp" => {
            if fields.len() != 3 {
                return Err(InputError {
                    field_index: 0,
                    message: "TCP Port Check requires a host, port, and timeout",
                });
            }
            let port = fields[1].parse::<u16>().map_err(|_| InputError {
                field_index: 1,
                message: "Port must be between 1 and 65535",
            })?;
            let timeout = fields[2].parse::<u64>().map_err(|_| InputError {
                field_index: 2,
                message: "Timeout must be between 1 and 60 seconds",
            })?;
            TcpTarget::new(fields[0].clone(), port, timeout)
                .map(ToolInput::Tcp)
                .map_err(|message| InputError {
                    field_index: if message.starts_with("Host") {
                        0
                    } else if message.starts_with("Port") {
                        1
                    } else {
                        2
                    },
                    message,
                })
        }
        _ => Err(InputError {
            field_index: 0,
            message: "This tool does not have an input form yet",
        }),
    }
}

pub async fn execute(
    input: ToolInput,
    cancel: impl Future<Output = Result<(), RunError>>,
) -> RunResult {
    let started = Instant::now();
    let tool_id = input.tool_id();
    let target = input.target();
    let tool = find_tool(tool_id);
    let max_timeout = tool.and_then(|tool| tool.max_timeout_seconds);
    let outcome = if !tool.is_some_and(|tool| platform_is_supported(tool, current_platform())) {
        Err(RunError::UnsupportedPlatform)
    } else {
        tokio::select! {
            biased;
            signal = cancel => Err(signal.err().unwrap_or(RunError::Cancelled)),
            result = async {
                if let Some(seconds) = max_timeout {
                    tokio::time::timeout(Duration::from_secs(seconds), run_tool(input))
                        .await
                        .unwrap_or(Err(RunError::DeadlineExceeded))
                } else {
                    run_tool(input).await
                }
            } => result,
        }
    };
    let (execution_status, finding, data, error) = match outcome {
        Ok((finding, data)) => (ExecutionStatus::Completed, Some(finding), data, None),
        Err(RunError::Cancelled) => (
            ExecutionStatus::Cancelled,
            None,
            None,
            Some(RunError::Cancelled),
        ),
        Err(error) => (ExecutionStatus::Failed, None, None, Some(error)),
    };
    RunResult {
        schema_version: 1,
        tool_id,
        target,
        execution_status,
        finding,
        data,
        duration_ms: started.elapsed().as_millis(),
        error,
    }
}

fn current_platform() -> Option<OperatingSystem> {
    match std::env::consts::OS {
        "windows" => Some(OperatingSystem::Windows),
        "linux" => Some(OperatingSystem::Linux),
        _ => None,
    }
}

fn platform_is_supported(tool: &ToolMetadata, platform: Option<OperatingSystem>) -> bool {
    platform.is_some_and(|platform| tool.platforms.contains(&platform))
}

async fn run_tool(input: ToolInput) -> Result<(Finding, Option<OutputData>), RunError> {
    match input {
        ToolInput::Tcp(target) => tcp::check(&target).await.map(|finding| (finding, None)),
        ToolInput::Dns(target) => {
            tools::network::dns(target.hostname, target.record_types, target.timeout_seconds)
                .await
                .map(|(found, data)| {
                    (
                        if found {
                            Finding::Available
                        } else {
                            Finding::Inconclusive
                        },
                        Some(OutputData::Dns(data)),
                    )
                })
        }
        ToolInput::ReverseDns(target) => {
            tools::network::reverse_dns(target.address, target.timeout_seconds)
                .await
                .map(|(found, data)| {
                    (
                        if found {
                            Finding::Available
                        } else {
                            Finding::Inconclusive
                        },
                        Some(OutputData::Dns(data)),
                    )
                })
        }
        ToolInput::Http(target) => tools::network::http(target.url, target.timeout_seconds)
            .await
            .map(|data| {
                let finding = if (200..300).contains(&data.status_code) {
                    Finding::Healthy
                } else {
                    Finding::Unhealthy
                };
                (finding, Some(OutputData::Http(data)))
            }),
        ToolInput::Tls(target) => tools::network::tls(target).await.map(|(trusted, data)| {
            (
                if trusted {
                    Finding::Available
                } else {
                    Finding::Unhealthy
                },
                Some(OutputData::Tls(data)),
            )
        }),
        ToolInput::Interfaces => tools::network::interfaces()
            .map(|data| (Finding::Available, Some(OutputData::Interfaces(data)))),
        ToolInput::Ping(target) => tools::network::ping(target).await.map(|(reachable, data)| {
            (
                if reachable {
                    Finding::Available
                } else {
                    Finding::Unreachable
                },
                Some(OutputData::Ping(data)),
            )
        }),
        ToolInput::SystemInfo => tools::system::system_info()
            .map(|data| (Finding::Available, Some(OutputData::SystemInfo(data)))),
        ToolInput::Cpu => tools::system::cpu()
            .await
            .map(|data| (Finding::Measured, Some(OutputData::Cpu(data)))),
        ToolInput::Memory => {
            tools::system::memory().map(|data| (Finding::Measured, Some(OutputData::Memory(data))))
        }
        ToolInput::Disk => {
            tools::system::disk().map(|data| (Finding::Measured, Some(OutputData::Disk(data))))
        }
        ToolInput::Sha256(target) => tools::system::sha256(target)
            .await
            .map(|data| (Finding::Computed, Some(OutputData::Sha256(data)))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn cancellation_has_no_diagnostic_finding() {
        let target = TcpTarget::new("127.0.0.1".into(), 443, 5).unwrap();
        let result = execute(ToolInput::Tcp(target), std::future::ready(Ok(()))).await;
        assert_eq!(result.exit_code(), 130);
        assert_eq!(result.error, Some(RunError::Cancelled));
        assert_eq!(result.finding, None);
        assert_eq!(result.execution_status_label(), "cancelled");
    }

    #[test]
    fn registry_has_complete_metadata_for_the_initial_tool_catalog() {
        assert_eq!(TOOLS.len(), 12);
        for tool in TOOLS {
            assert!(!tool.id.is_empty());
            assert!(tool.version > 0);
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(!tool.keywords.is_empty());
            assert!(!tool.aliases.is_empty());
            assert!(!tool.sample_queries.is_empty());
            assert_eq!(
                tool.sample_inputs.len(),
                tool.input_fields.len(),
                "{}",
                tool.id
            );
            assert!(!tool.platforms.is_empty());
            assert!(!tool.help.is_empty());
            assert!(!tool.output_fields.is_empty());
            assert!(!tool.output_schema.to_string().is_empty());
            assert!(!tool.status_explanation.is_empty());
            assert!(tool.max_targets > 0);
            assert!(tool.max_parallel_operations > 0);
        }
        assert_eq!(
            find_tool("network.dns").unwrap().platform_status,
            PlatformStatus::NeedsSmokeTest
        );
        assert_eq!(
            find_tool("network.ping").unwrap().backend,
            ExecutionBackend::OperatingSystem
        );
    }

    #[test]
    fn executor_platform_gate_uses_registry_targets() {
        let tcp = find_tool("network.tcp").unwrap();
        assert!(platform_is_supported(tcp, Some(OperatingSystem::Windows)));
        assert!(platform_is_supported(tcp, Some(OperatingSystem::Linux)));
        assert!(!platform_is_supported(tcp, None));
        assert_eq!(tcp.smoke_tested_platforms, &[OperatingSystem::Windows]);
        assert_eq!(tcp.platform_status, PlatformStatus::NeedsSmokeTest);
    }

    #[test]
    fn network_inputs_are_validated_and_sensitive_url_parts_are_not_echoed() {
        assert!(
            parse_tool_input(
                "network.dns",
                &["example.com".into(), "A,AAAA".into(), "5".into()]
            )
            .is_ok()
        );
        assert!(
            parse_tool_input("network.dns", &["127.0.0.1".into(), "A".into(), "5".into()]).is_err()
        );
        assert!(
            parse_tool_input("network.reverse_dns", &["2001:db8::1".into(), "5".into()]).is_ok()
        );
        assert!(
            parse_tool_input(
                "network.ping",
                &["example.com".into(), "11".into(), "5".into()]
            )
            .is_err()
        );
        let input = parse_tool_input(
            "web.http",
            &[
                "https://example.com/check?token=hidden#section".into(),
                "10".into(),
            ],
        )
        .unwrap();
        assert_eq!(input.target(), "https://example.com/check");
        let tls = parse_tool_input(
            "security.tls",
            &[
                "https://example.com/certificate?token=hidden".into(),
                "443".into(),
                "5".into(),
            ],
        )
        .unwrap();
        assert_eq!(tls.target(), "example.com:443");
        assert!(
            parse_tool_input(
                "security.tls",
                &["http://example.com".into(), "443".into(), "5".into()]
            )
            .is_err()
        );
        assert!(
            parse_tool_input(
                "security.tls",
                &["https://example.com:8443".into(), "443".into(), "5".into()]
            )
            .is_err()
        );
    }

    #[test]
    fn registry_bounds_drive_shared_numeric_validation() {
        assert!(validate_numeric_input("network.tcp", "port", 1).is_ok());
        assert!(validate_numeric_input("network.tcp", "port", 0).is_err());
        assert!(validate_numeric_input("network.tcp", "timeout", 60).is_ok());
        assert!(validate_numeric_input("network.tcp", "timeout", 61).is_err());
        let error = parse_tool_input(
            "network.ping",
            &["example.com".into(), "11".into(), "5".into()],
        )
        .err()
        .unwrap();
        assert_eq!(error.field_index, 1);
    }
}
