use clap::{ColorChoice, Parser, Subcommand, ValueEnum};
use nyra::{OutputData, RunError, TOOLS, ToolInput, execute, parse_tool_input};
use std::{
    io::{self, IsTerminal, Write},
    process::ExitCode,
};

#[derive(Parser)]
#[command(version, about, color = ColorChoice::Never)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Discover available diagnostics.
    Tools {
        #[command(subcommand)]
        command: ToolsCommand,
    },
    /// Run network diagnostics.
    Net {
        #[command(subcommand)]
        command: NetCommand,
    },
    /// Check a website over HTTP or HTTPS.
    Web {
        #[command(subcommand)]
        command: WebCommand,
    },
    /// Inspect a TLS endpoint certificate.
    Tls {
        #[command(subcommand)]
        command: TlsCommand,
    },
    /// Inspect the local system.
    Sys {
        #[command(subcommand)]
        command: SysCommand,
    },
    /// Run file integrity diagnostics.
    File {
        #[command(subcommand)]
        command: FileCommand,
    },
}

#[derive(Subcommand)]
enum ToolsCommand {
    /// List implemented tools.
    List,
}

#[derive(Subcommand)]
enum NetCommand {
    /// Check whether a single target accepts a TCP connection.
    Tcp {
        host: String,
        port: u16,
        /// Total DNS and connection budget in seconds, from 1 to 60.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// Query DNS records using the system resolver.
    Dns {
        hostname: String,
        /// Comma-separated A, AAAA, CNAME, MX, and TXT record types.
        #[arg(long = "record-types", default_value = "A,AAAA")]
        record_types: String,
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// Find PTR records for an IP address.
    ReverseDns {
        address: String,
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// Send a bounded set of ICMP echo requests.
    Ping {
        host: String,
        #[arg(long, default_value_t = 4)]
        count: u8,
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// List local interface addresses and netmasks.
    Interfaces {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
}

#[derive(Subcommand)]
enum WebCommand {
    /// Request one URL and report status and selected headers.
    Check {
        url: String,
        #[arg(long, default_value_t = 10)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
}

#[derive(Subcommand)]
enum TlsCommand {
    /// Inspect a TLS leaf certificate and report system validation status.
    Inspect {
        host: String,
        #[arg(long, default_value_t = 443)]
        port: u16,
        #[arg(long, default_value_t = 10)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
}

#[derive(Subcommand)]
enum SysCommand {
    /// Show operating system and host information.
    Info {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// Show processor details and utilization.
    Cpu {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// Show memory totals.
    Memory {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
    /// List local volumes and available space.
    Disk {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
}

#[derive(Subcommand)]
enum FileCommand {
    /// Compute a file's SHA-256 digest.
    Sha256 {
        path: String,
        /// Total read budget in seconds, from 1 to 60.
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
    },
}

#[derive(Clone, ValueEnum)]
enum Output {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            return ExitCode::from(if error.print().is_ok() { code } else { 3 });
        }
    };
    match cli.command {
        Some(command) => match run(command) {
            Ok(code) => ExitCode::from(code),
            Err(error) => {
                eprintln!("Error: {error}");
                ExitCode::from(3)
            }
        },
        None if io::stdin().is_terminal() && io::stdout().is_terminal() => match nyra::tui::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Error: {error}");
                ExitCode::from(3)
            }
        },
        None => {
            eprintln!(
                "No command provided. NYRA's interactive mode requires an interactive terminal; run `nyra --help` for CLI commands."
            );
            ExitCode::from(2)
        }
    }
}

fn run(command: Command) -> io::Result<u8> {
    let mut stdout = io::stdout().lock();
    match command {
        Command::Tools {
            command: ToolsCommand::List,
        } => {
            writeln!(
                stdout,
                "ID\tNAME\tCATEGORY\tRISK\tBACKEND\tPLATFORMS\tSTATUS\tDESCRIPTION"
            )?;
            for tool in TOOLS {
                let platforms = tool
                    .platforms
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    stdout,
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    tool.id,
                    tool.name,
                    tool.category,
                    tool.risk,
                    tool.backend,
                    platforms,
                    tool.platform_status,
                    tool.description
                )?;
            }
            Ok(0)
        }
        Command::Net {
            command:
                NetCommand::Tcp {
                    host,
                    port,
                    timeout,
                    output,
                },
        } => parsed_check(
            &mut stdout,
            "network.tcp",
            vec![host, port.to_string(), timeout.to_string()],
            output,
        ),
        Command::Net {
            command:
                NetCommand::Dns {
                    hostname,
                    record_types,
                    timeout,
                    output,
                },
        } => parsed_check(
            &mut stdout,
            "network.dns",
            vec![hostname, record_types, timeout.to_string()],
            output,
        ),
        Command::Net {
            command:
                NetCommand::ReverseDns {
                    address,
                    timeout,
                    output,
                },
        } => parsed_check(
            &mut stdout,
            "network.reverse_dns",
            vec![address, timeout.to_string()],
            output,
        ),
        Command::Net {
            command:
                NetCommand::Ping {
                    host,
                    count,
                    timeout,
                    output,
                },
        } => parsed_check(
            &mut stdout,
            "network.ping",
            vec![host, count.to_string(), timeout.to_string()],
            output,
        ),
        Command::Net {
            command: NetCommand::Interfaces { output },
        } => parsed_check(&mut stdout, "network.interfaces", Vec::new(), output),
        Command::Web {
            command:
                WebCommand::Check {
                    url,
                    timeout,
                    output,
                },
        } => parsed_check(
            &mut stdout,
            "web.http",
            vec![url, timeout.to_string()],
            output,
        ),
        Command::Tls {
            command:
                TlsCommand::Inspect {
                    host,
                    port,
                    timeout,
                    output,
                },
        } => parsed_check(
            &mut stdout,
            "security.tls",
            vec![host, port.to_string(), timeout.to_string()],
            output,
        ),
        Command::Sys {
            command: SysCommand::Info { output },
        } => run_check(&mut stdout, ToolInput::SystemInfo, output),
        Command::Sys {
            command: SysCommand::Cpu { output },
        } => run_check(&mut stdout, ToolInput::Cpu, output),
        Command::Sys {
            command: SysCommand::Memory { output },
        } => run_check(&mut stdout, ToolInput::Memory, output),
        Command::Sys {
            command: SysCommand::Disk { output },
        } => run_check(&mut stdout, ToolInput::Disk, output),
        Command::File {
            command:
                FileCommand::Sha256 {
                    path,
                    timeout,
                    output,
                },
        } => parsed_check(
            &mut stdout,
            "file.sha256",
            vec![path, timeout.to_string()],
            output,
        ),
    }
}

fn parsed_check(
    stdout: &mut impl Write,
    tool_id: &str,
    fields: Vec<String>,
    output: Output,
) -> io::Result<u8> {
    let input = match parse_tool_input(tool_id, &fields) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("Error: {}", error.message);
            return Ok(2);
        }
    };
    run_check(stdout, input, output)
}

fn run_check(stdout: &mut impl Write, input: ToolInput, output: Output) -> io::Result<u8> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(execute(input, async {
        tokio::signal::ctrl_c()
            .await
            .map_err(|_| RunError::SignalUnavailable)
    }));
    // A system DNS call may still be running on Tokio's blocking pool.
    runtime.shutdown_background();
    let exit_code = result.exit_code();
    match output {
        Output::Json => {
            serde_json::to_writer(&mut *stdout, &result)?;
            writeln!(stdout)?;
        }
        Output::Text => {
            if let Some(finding) = result.finding {
                writeln!(
                    stdout,
                    "{}: {finding} ({} ms)",
                    result.target, result.duration_ms
                )?;
            }
            if let Some(data) = result.data {
                match data {
                    OutputData::SystemInfo(info) => {
                        writeln!(
                            stdout,
                            "OS: {} ({})",
                            info.os_name.as_deref().unwrap_or("unknown"),
                            info.os
                        )?;
                        writeln!(stdout, "Architecture: {}", info.architecture)?;
                        if let Some(hostname) = info.hostname {
                            writeln!(stdout, "Host: {hostname}")?;
                        }
                        if let Some(uptime) = info.uptime_seconds {
                            writeln!(stdout, "Uptime: {uptime} seconds")?;
                        }
                        if let Some(version) = info.os_version {
                            writeln!(stdout, "OS version: {version}")?;
                        }
                        if let Some(version) = info.kernel_version {
                            writeln!(stdout, "Kernel: {version}")?;
                        }
                    }
                    OutputData::Dns(info) => {
                        writeln!(stdout, "DNS records for {}", info.hostname)?;
                        for record in info.records {
                            if record.answers.is_empty() {
                                writeln!(stdout, "{}: no records", record.record_type)?;
                            } else {
                                for answer in record.answers {
                                    writeln!(stdout, "{}: {answer}", record.record_type)?;
                                }
                            }
                        }
                    }
                    OutputData::Http(info) => {
                        writeln!(stdout, "HTTP status: {}", info.status_code)?;
                        writeln!(stdout, "Response headers in {} ms", info.elapsed_ms)?;
                        for (name, value) in info.headers {
                            writeln!(stdout, "{name}: {value}")?;
                        }
                    }
                    OutputData::Tls(info) => {
                        writeln!(stdout, "TLS certificate: {}:{}", info.host, info.port)?;
                        writeln!(stdout, "Subject: {}", info.subject)?;
                        writeln!(stdout, "Issuer: {}", info.issuer)?;
                        writeln!(stdout, "Valid from: {}", info.not_before_unix)?;
                        writeln!(stdout, "Valid until: {}", info.not_after_unix)?;
                        writeln!(stdout, "Trust: {}", info.trust_validation)?;
                        if !info.subject_alt_names.is_empty() {
                            writeln!(
                                stdout,
                                "Subject alternative names: {}",
                                info.subject_alt_names.join(", ")
                            )?;
                        }
                    }
                    OutputData::Interfaces(info) => {
                        for interface in info.interfaces {
                            writeln!(
                                stdout,
                                "{}: {}/{}{}",
                                interface.name,
                                interface.address,
                                interface.netmask,
                                if interface.loopback {
                                    " (loopback)"
                                } else {
                                    ""
                                }
                            )?;
                        }
                    }
                    OutputData::Ping(info) => {
                        writeln!(
                            stdout,
                            "{}: {}",
                            info.host,
                            if info.reachable {
                                "reachable"
                            } else {
                                "no ICMP reply"
                            }
                        )?;
                        if let Some(latency) = info.latency_ms {
                            writeln!(stdout, "Round trip: {latency:.2} ms")?;
                        }
                        writeln!(stdout, "{}", info.note)?;
                    }
                    OutputData::Cpu(info) => {
                        writeln!(
                            stdout,
                            "Processor: {}",
                            info.brand.as_deref().unwrap_or("unknown")
                        )?;
                        writeln!(
                            stdout,
                            "Cores: {} logical, {} physical",
                            info.logical_cores,
                            info.physical_cores
                                .map_or_else(|| "unknown".into(), |count| count.to_string())
                        )?;
                        if let Some(usage) = info.average_usage_percent {
                            writeln!(stdout, "Sampled CPU usage: {usage:.1}%")?;
                        }
                        if let Some(frequency) = info.frequency_mhz {
                            writeln!(stdout, "Reported frequency: {frequency} MHz")?;
                        }
                    }
                    OutputData::Memory(info) => {
                        writeln!(stdout, "Total: {} bytes", info.total_bytes)?;
                        writeln!(stdout, "Used: {} bytes", info.used_bytes)?;
                        writeln!(stdout, "Available: {} bytes", info.available_bytes)?;
                    }
                    OutputData::Disk(info) => {
                        for volume in info.volumes {
                            writeln!(stdout, "{} ({})", volume.mount_point, volume.file_system)?;
                            writeln!(stdout, "  Total: {} bytes", volume.total_bytes)?;
                            writeln!(stdout, "  Available: {} bytes", volume.available_bytes)?;
                        }
                    }
                    OutputData::Sha256(info) => {
                        writeln!(stdout, "SHA-256: {}", info.sha256)?;
                        writeln!(stdout, "Size: {} bytes", info.size_bytes)?;
                        writeln!(stdout, "File: {}", info.path)?;
                    }
                }
            }
        }
    }
    if let Some(error) = result.error {
        eprintln!("Error: {error}");
    }
    stdout.flush()?;
    Ok(exit_code)
}
