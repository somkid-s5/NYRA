use clap::{Args, ColorChoice, Parser, Subcommand, ValueEnum};
use nyra::{
    CATALOG, CsvReportDestination, CustomThemeFileError, Finding, HtmlReportDestination, InputKind,
    JsonReportDestination, KNOWLEDGE_CARDS, OutputData, ReportExportError, RunError, RunResult,
    ToolInput, execute, export_custom_theme_file, find_knowledge_card, find_tool,
    import_custom_theme_file, parse_tool_input, search_knowledge_cards, validate_custom_theme_file,
};
use nyra::{
    workflow_definition::{
        WorkflowDefinition, WorkflowFinding, WorkflowStepStatus, WorkflowValue, WorkflowValueType,
    },
    workflow_registry::workflow_tool_catalog,
    workflow_runner::{
        WorkflowExecutionStatus, WorkflowRunError, WorkflowRunIssue, WorkflowRunResult,
        WorkflowStepExecutor, WorkflowToolFuture, WorkflowToolOutcome, resolve_inputs,
        run_workflow,
    },
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

#[derive(Parser)]
#[command(version, about, color = ColorChoice::Never)]
struct Cli {
    /// Enable optional diagnostic logs on stderr. Logs omit targets and input values.
    #[arg(long, global = true, value_enum, default_value = "off")]
    log_level: LogLevel,
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
    /// Run guided multi-step diagnostics.
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommand,
    },
    /// Validate, install, and export custom appearance themes.
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
    /// Browse offline system-administration command references.
    Knowledge {
        #[command(subcommand)]
        command: KnowledgeCommand,
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
        #[command(flatten)]
        report: ReportOptions,
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
        #[command(flatten)]
        report: ReportOptions,
    },
    /// Find PTR records for an IP address.
    ReverseDns {
        address: String,
        #[arg(long, default_value_t = 5)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        #[command(flatten)]
        report: ReportOptions,
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
        #[command(flatten)]
        report: ReportOptions,
    },
    /// List local interface addresses and netmasks.
    Interfaces {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        #[command(flatten)]
        report: ReportOptions,
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
        #[command(flatten)]
        report: ReportOptions,
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
        #[command(flatten)]
        report: ReportOptions,
    },
}

#[derive(Subcommand)]
enum SysCommand {
    /// Show operating system and host information.
    Info {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        #[command(flatten)]
        report: ReportOptions,
    },
    /// Show processor details and utilization.
    Cpu {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        #[command(flatten)]
        report: ReportOptions,
    },
    /// Show memory totals.
    Memory {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        #[command(flatten)]
        report: ReportOptions,
    },
    /// List local volumes and available space.
    Disk {
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        #[command(flatten)]
        report: ReportOptions,
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
        #[command(flatten)]
        report: ReportOptions,
    },
}

#[derive(Subcommand)]
enum WorkflowCommand {
    /// Run a bundled diagnostic workflow.
    Run {
        /// Workflow identifier. The initial supported workflow is web-unreachable.
        workflow_id: String,
        /// HTTP or HTTPS target URL.
        #[arg(long)]
        target: String,
        #[arg(long, default_value_t = 15)]
        timeout: u64,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        /// Write a redacted JSON report to a new file. Existing files are never replaced.
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with_all = ["report_html", "report_csv"]
        )]
        report_json: Option<PathBuf>,
        /// Write a redacted HTML report to a new file. Existing files are never replaced.
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with_all = ["report_json", "report_csv"]
        )]
        report_html: Option<PathBuf>,
        /// Write a redacted CSV report to a new file. Existing files are never replaced.
        #[arg(
            long,
            value_name = "PATH",
            conflicts_with_all = ["report_json", "report_html"]
        )]
        report_csv: Option<PathBuf>,
    },
    /// Validate a user-defined typed workflow file without running diagnostics.
    Validate { path: PathBuf },
    /// Run a user-defined sequential workflow through the shared tool executor.
    RunFile {
        path: PathBuf,
        /// Supply a declared input as NAME=VALUE. May be repeated.
        #[arg(long = "input", value_name = "NAME=VALUE", action = clap::ArgAction::Append)]
        inputs: Vec<String>,
        #[arg(long, value_enum, default_value = "text")]
        output: Output,
        #[command(flatten)]
        reports: ReportOptions,
    },
}

#[derive(Subcommand)]
enum ThemeCommand {
    /// Validate a version 1 custom theme TOML file.
    Validate { path: PathBuf },
    /// Install a validated custom theme into NYRA's active theme directory.
    Import {
        source: PathBuf,
        /// Replace the installed custom theme if one already exists.
        #[arg(long)]
        force: bool,
    },
    /// Export the installed custom theme to a file.
    Export {
        destination: PathBuf,
        /// Replace the destination file if it already exists.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum KnowledgeCommand {
    /// List bundled offline command references.
    List,
    /// Search card titles, intent, syntax, and curated English keywords.
    Search {
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
    },
    /// Show a command reference by stable ID.
    Show { card_id: String },
}

#[derive(Clone, ValueEnum)]
enum Output {
    Text,
    Json,
}

#[derive(Args, Default)]
struct ReportOptions {
    /// Write a sanitized JSON report to a new file. Existing files are never replaced.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = ["report_html", "report_csv"]
    )]
    report_json: Option<PathBuf>,
    /// Write a sanitized HTML report to a new file. Existing files are never replaced.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = ["report_json", "report_csv"]
    )]
    report_html: Option<PathBuf>,
    /// Write a sanitized CSV report to a new file. Existing files are never replaced.
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with_all = ["report_json", "report_html"]
    )]
    report_csv: Option<PathBuf>,
}

enum ReportDestination {
    Json(JsonReportDestination),
    Html(HtmlReportDestination),
    Csv(CsvReportDestination),
}

const MAX_WORKFLOW_FILE_BYTES: usize = 64 * 1024;

#[derive(Debug)]
enum WorkflowFileError {
    Unavailable,
    TooLarge,
    InvalidEncoding,
    InvalidDocument,
}

impl std::fmt::Display for WorkflowFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "workflow file is unavailable",
            Self::TooLarge => "workflow file exceeds the 64 KiB limit",
            Self::InvalidEncoding => "workflow file must use UTF-8",
            Self::InvalidDocument => "workflow file is not valid TOML",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkflowInputError {
    Malformed,
    Unknown,
    Duplicate,
    InvalidValue,
}

impl std::fmt::Display for WorkflowInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Malformed => "workflow input must use NAME=VALUE",
            Self::Unknown => "workflow input name is not declared",
            Self::Duplicate => "workflow input was supplied more than once",
            Self::InvalidValue => "workflow input value has the wrong type",
        })
    }
}

impl ReportDestination {
    fn success_message(&self) -> &'static str {
        match self {
            Self::Json(_) => "JSON report exported.",
            Self::Html(_) => "HTML report exported.",
            Self::Csv(_) => "CSV report exported.",
        }
    }

    fn write(self, result: &RunResult) -> Result<(), ReportExportError> {
        match self {
            Self::Json(destination) => destination.write(result),
            Self::Html(destination) => destination.write(result),
            Self::Csv(destination) => destination.write(result),
        }
    }

    fn write_workflow_run(self, result: &WorkflowRunResult) -> Result<(), ReportExportError> {
        match self {
            Self::Json(destination) => destination.write_workflow_run(result),
            Self::Html(destination) => destination.write_workflow_run(result),
            Self::Csv(destination) => destination.write_workflow_run(result),
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
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
        Some(command) => {
            init_tracing(cli.log_level);
            match run(command) {
                Ok(code) => ExitCode::from(code),
                Err(error) => {
                    eprintln!("Error: {error}");
                    ExitCode::from(3)
                }
            }
        }
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

fn init_tracing(level: LogLevel) {
    let level = match level {
        LogLevel::Off => return,
        LogLevel::Error => "error",
        LogLevel::Warn => "warn",
        LogLevel::Info => "info",
        LogLevel::Debug => "debug",
    };
    let filter = tracing_subscriber::EnvFilter::new(format!("nyra={level}"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .try_init();
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
            for tool in CATALOG {
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
                    report,
                },
        } => parsed_check(
            &mut stdout,
            "network.tcp",
            vec![host, port.to_string(), timeout.to_string()],
            output,
            report,
        ),
        Command::Net {
            command:
                NetCommand::Dns {
                    hostname,
                    record_types,
                    timeout,
                    output,
                    report,
                },
        } => parsed_check(
            &mut stdout,
            "network.dns",
            vec![hostname, record_types, timeout.to_string()],
            output,
            report,
        ),
        Command::Net {
            command:
                NetCommand::ReverseDns {
                    address,
                    timeout,
                    output,
                    report,
                },
        } => parsed_check(
            &mut stdout,
            "network.reverse_dns",
            vec![address, timeout.to_string()],
            output,
            report,
        ),
        Command::Net {
            command:
                NetCommand::Ping {
                    host,
                    count,
                    timeout,
                    output,
                    report,
                },
        } => parsed_check(
            &mut stdout,
            "network.ping",
            vec![host, count.to_string(), timeout.to_string()],
            output,
            report,
        ),
        Command::Net {
            command: NetCommand::Interfaces { output, report },
        } => parsed_check(
            &mut stdout,
            "network.interfaces",
            Vec::new(),
            output,
            report,
        ),
        Command::Web {
            command:
                WebCommand::Check {
                    url,
                    timeout,
                    output,
                    report,
                },
        } => parsed_check(
            &mut stdout,
            "web.http",
            vec![url, timeout.to_string()],
            output,
            report,
        ),
        Command::Tls {
            command:
                TlsCommand::Inspect {
                    host,
                    port,
                    timeout,
                    output,
                    report,
                },
        } => parsed_check(
            &mut stdout,
            "security.tls",
            vec![host, port.to_string(), timeout.to_string()],
            output,
            report,
        ),
        Command::Sys {
            command: SysCommand::Info { output, report },
        } => run_check(&mut stdout, ToolInput::SystemInfo, output, report),
        Command::Sys {
            command: SysCommand::Cpu { output, report },
        } => run_check(&mut stdout, ToolInput::Cpu, output, report),
        Command::Sys {
            command: SysCommand::Memory { output, report },
        } => run_check(&mut stdout, ToolInput::Memory, output, report),
        Command::Sys {
            command: SysCommand::Disk { output, report },
        } => run_check(&mut stdout, ToolInput::Disk, output, report),
        Command::File {
            command:
                FileCommand::Sha256 {
                    path,
                    timeout,
                    output,
                    report,
                },
        } => parsed_check(
            &mut stdout,
            "file.sha256",
            vec![path, timeout.to_string()],
            output,
            report,
        ),
        Command::Workflow {
            command:
                WorkflowCommand::Run {
                    workflow_id,
                    target,
                    timeout,
                    output,
                    report_json,
                    report_html,
                    report_csv,
                },
        } => {
            if workflow_id != "web-unreachable" {
                eprintln!("Error: unknown workflow; available workflow: web-unreachable");
                return Ok(2);
            }
            run_workflow_check(
                &mut stdout,
                target,
                timeout,
                output,
                report_json,
                report_html,
                report_csv,
            )
        }
        Command::Workflow {
            command: WorkflowCommand::Validate { path },
        } => validate_workflow_file(&mut stdout, &path),
        Command::Workflow {
            command:
                WorkflowCommand::RunFile {
                    path,
                    inputs,
                    output,
                    reports,
                },
        } => run_workflow_file(&mut stdout, &path, &inputs, output, reports),
        Command::Theme { command } => run_theme(&mut stdout, command),
        Command::Knowledge { command } => run_knowledge(&mut stdout, command),
    }
}

fn validate_workflow_file(stdout: &mut impl Write, path: &Path) -> io::Result<u8> {
    let definition = match load_workflow_file(path) {
        Ok(definition) => definition,
        Err(error) => {
            writeln!(io::stderr().lock(), "Error: {error}")?;
            return Ok(2);
        }
    };
    match definition.validate(&workflow_tool_catalog()) {
        Ok(()) => {
            writeln!(stdout, "Workflow definition is valid.")?;
            Ok(0)
        }
        Err(errors) => {
            writeln!(
                io::stderr().lock(),
                "Error: workflow definition is invalid ({} validation errors).",
                errors.len()
            )?;
            Ok(2)
        }
    }
}

fn load_workflow_file(path: &Path) -> Result<WorkflowDefinition, WorkflowFileError> {
    let metadata = std::fs::metadata(path).map_err(|_| WorkflowFileError::Unavailable)?;
    if !metadata.is_file() {
        return Err(WorkflowFileError::Unavailable);
    }
    let file = File::open(path).map_err(|_| WorkflowFileError::Unavailable)?;
    let mut bytes = Vec::with_capacity(MAX_WORKFLOW_FILE_BYTES.min(8 * 1024));
    file.take((MAX_WORKFLOW_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| WorkflowFileError::Unavailable)?;
    if bytes.len() > MAX_WORKFLOW_FILE_BYTES {
        return Err(WorkflowFileError::TooLarge);
    }
    let contents = String::from_utf8(bytes).map_err(|_| WorkflowFileError::InvalidEncoding)?;
    toml_edit::de::from_str(&contents).map_err(|_| WorkflowFileError::InvalidDocument)
}

fn run_workflow_file(
    stdout: &mut impl Write,
    path: &Path,
    supplied_inputs: &[String],
    output: Output,
    reports: ReportOptions,
) -> io::Result<u8> {
    let definition = match load_workflow_file(path) {
        Ok(definition) => definition,
        Err(error) => {
            writeln!(io::stderr().lock(), "Error: {error}")?;
            return Ok(2);
        }
    };
    let schemas = workflow_tool_catalog();
    if let Err(errors) = definition.validate(&schemas) {
        writeln!(
            io::stderr().lock(),
            "Error: workflow definition is invalid ({} validation errors).",
            errors.len()
        )?;
        return Ok(2);
    }
    let inputs = match parse_workflow_inputs(&definition, supplied_inputs) {
        Ok(inputs) => inputs,
        Err(error) => {
            writeln!(io::stderr().lock(), "Error: {error}")?;
            return Ok(2);
        }
    };

    let inputs = match resolve_inputs(&definition, inputs) {
        Ok(inputs) => inputs,
        Err(error) => {
            writeln!(io::stderr().lock(), "Error: {error}")?;
            return Ok(2);
        }
    };
    let destination = match prepare_report_destination(reports) {
        Ok(destination) => destination,
        Err(error) => {
            writeln!(io::stderr().lock(), "Error: {error}")?;
            return Ok(error.exit_code());
        }
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        let (cancel_sender, cancel_receiver) = tokio::sync::oneshot::channel();
        let workflow = run_workflow(
            &definition,
            &schemas,
            inputs,
            &RegisteredWorkflowStepExecutor,
            async move {
                let _ = cancel_receiver.await;
            },
            None,
        );
        tokio::pin!(workflow);
        tokio::select! {
            biased;
            signal = tokio::signal::ctrl_c() => match signal {
                Ok(()) => {
                    let _ = cancel_sender.send(());
                    Ok(workflow.await)
                }
                Err(_) => Err(()),
            },
            result = &mut workflow => Ok(result),
        }
    });
    runtime.shutdown_background();

    let result = match result {
        Err(()) => {
            writeln!(
                io::stderr().lock(),
                "Error: unable to register the interrupt handler."
            )?;
            return Ok(3);
        }
        Ok(Err(error)) => {
            let message = match error {
                WorkflowRunError::InvalidDefinition(ref errors) => format!(
                    "workflow definition is invalid ({} validation errors)",
                    errors.len()
                ),
                WorkflowRunError::MissingRequiredInput => {
                    "a required workflow input is missing".to_owned()
                }
                WorkflowRunError::UnknownInput => {
                    "an undeclared workflow input was supplied".to_owned()
                }
                WorkflowRunError::InvalidInputValue => {
                    "a workflow input value is invalid".to_owned()
                }
            };
            writeln!(io::stderr().lock(), "Error: {message}")?;
            return Ok(2);
        }
        Ok(Ok(result)) => result,
    };
    let exit_code = workflow_exit_code(&result);
    match output {
        Output::Json => {
            serde_json::to_writer(&mut *stdout, &result)?;
            writeln!(stdout)?;
        }
        Output::Text => write_workflow_result(stdout, &result)?,
    }
    if let Some(destination) = destination {
        let message = destination.success_message();
        match destination.write_workflow_run(&result) {
            Ok(()) => writeln!(io::stderr().lock(), "{message}")?,
            Err(error) => {
                writeln!(io::stderr().lock(), "Error: {error}")?;
                return Ok(error.exit_code());
            }
        }
    }
    Ok(exit_code)
}

fn parse_workflow_inputs(
    definition: &WorkflowDefinition,
    supplied: &[String],
) -> Result<BTreeMap<String, WorkflowValue>, WorkflowInputError> {
    let mut parsed = BTreeMap::new();
    for argument in supplied {
        let (name, value) = argument
            .split_once('=')
            .ok_or(WorkflowInputError::Malformed)?;
        let input = definition
            .inputs
            .iter()
            .find(|input| input.name == name)
            .ok_or(WorkflowInputError::Unknown)?;
        if parsed.contains_key(name) {
            return Err(WorkflowInputError::Duplicate);
        }
        let typed_value = match input.value_type {
            WorkflowValueType::String
            | WorkflowValueType::DnsHostname
            | WorkflowValueType::Hostname
            | WorkflowValueType::IpAddress
            | WorkflowValueType::Url
            | WorkflowValueType::FilePath
            | WorkflowValueType::RecordTypes => WorkflowValue::Text(value.to_owned()),
            WorkflowValueType::Port
            | WorkflowValueType::TimeoutSeconds
            | WorkflowValueType::Count
            | WorkflowValueType::Integer => WorkflowValue::Integer(
                value
                    .parse::<i64>()
                    .map_err(|_| WorkflowInputError::InvalidValue)?,
            ),
            WorkflowValueType::Boolean => WorkflowValue::Boolean(match value {
                "true" => true,
                "false" => false,
                _ => return Err(WorkflowInputError::InvalidValue),
            }),
        };
        parsed.insert(name.to_owned(), typed_value);
    }
    Ok(parsed)
}

struct RegisteredWorkflowStepExecutor;

impl WorkflowStepExecutor for RegisteredWorkflowStepExecutor {
    fn execute<'a>(
        &'a self,
        tool_id: &'a str,
        parameters: BTreeMap<String, WorkflowValue>,
        timeout: Duration,
    ) -> WorkflowToolFuture<'a> {
        let prepared = prepare_registered_tool_input(tool_id, parameters, timeout);
        Box::pin(async move {
            let input = match prepared {
                Ok(input) => input,
                Err(()) => {
                    return WorkflowToolOutcome::new(
                        WorkflowStepStatus::Failed,
                        None,
                        BTreeMap::new(),
                    );
                }
            };
            let result = execute(input, std::future::pending::<Result<(), RunError>>()).await;
            let status = match result.execution_status_label() {
                "completed" => WorkflowStepStatus::Completed,
                "cancelled" => WorkflowStepStatus::Cancelled,
                _ => WorkflowStepStatus::Failed,
            };
            let finding = result.finding.map(workflow_finding);
            let outputs = workflow_output_values(result.data);
            WorkflowToolOutcome::new(status, finding, outputs)
        })
    }
}

fn prepare_registered_tool_input(
    tool_id: &str,
    parameters: BTreeMap<String, WorkflowValue>,
    timeout: Duration,
) -> Result<ToolInput, ()> {
    let tool = find_tool(tool_id).ok_or(())?;
    let mut fields = Vec::with_capacity(tool.input_fields.len());
    for field in tool.input_fields {
        let value = match parameters.get(field.name) {
            Some(value) => workflow_parameter_text(value)?,
            None => field.default.map(str::to_owned).unwrap_or_default(),
        };
        let bounded_value = if matches!(field.kind, InputKind::TimeoutSeconds) {
            let requested = value.parse::<u64>().map_err(|_| ())?;
            let remaining_seconds = timeout.as_secs().max(1);
            requested.min(remaining_seconds).to_string()
        } else {
            value
        };
        fields.push(bounded_value);
    }
    parse_tool_input(tool_id, &fields).map_err(|_| ())
}

fn workflow_parameter_text(value: &WorkflowValue) -> Result<String, ()> {
    match value {
        WorkflowValue::Text(value) => Ok(value.clone()),
        WorkflowValue::Integer(value) => Ok(value.to_string()),
        WorkflowValue::Boolean(value) => Ok(value.to_string()),
    }
}

fn workflow_finding(finding: Finding) -> WorkflowFinding {
    match finding {
        Finding::Open => WorkflowFinding::Open,
        Finding::Refused => WorkflowFinding::Refused,
        Finding::TimedOut => WorkflowFinding::TimedOut,
        Finding::Unreachable => WorkflowFinding::Unreachable,
        Finding::Inconclusive => WorkflowFinding::Inconclusive,
        Finding::Available => WorkflowFinding::Available,
        Finding::Measured => WorkflowFinding::Measured,
        Finding::Computed => WorkflowFinding::Computed,
        Finding::Healthy => WorkflowFinding::Healthy,
        Finding::Unhealthy => WorkflowFinding::Unhealthy,
    }
}

fn workflow_output_values(data: Option<OutputData>) -> BTreeMap<String, WorkflowValue> {
    let mut outputs = BTreeMap::new();
    let Some(data) = data else {
        return outputs;
    };
    match data {
        OutputData::Dns(data) => insert_workflow_text(&mut outputs, "hostname", data.hostname),
        OutputData::Http(data) => {
            insert_workflow_text(&mut outputs, "url", data.url);
            insert_workflow_integer(&mut outputs, "status_code", i64::from(data.status_code));
            insert_workflow_u128(&mut outputs, "elapsed_ms", data.elapsed_ms);
        }
        OutputData::Tls(data) => {
            insert_workflow_text(&mut outputs, "host", data.host);
            insert_workflow_integer(&mut outputs, "port", i64::from(data.port));
            insert_workflow_text(&mut outputs, "subject", data.subject);
            insert_workflow_text(&mut outputs, "issuer", data.issuer);
            insert_workflow_integer(&mut outputs, "not_before_unix", data.not_before_unix);
            insert_workflow_integer(&mut outputs, "not_after_unix", data.not_after_unix);
            insert_workflow_text(&mut outputs, "trust_validation", data.trust_validation);
        }
        OutputData::Ping(data) => {
            insert_workflow_text(&mut outputs, "host", data.host);
            insert_workflow_integer(&mut outputs, "count", i64::from(data.count));
            outputs.insert(
                "reachable".to_owned(),
                WorkflowValue::Boolean(data.reachable),
            );
        }
        OutputData::SystemInfo(data) => {
            insert_workflow_text(&mut outputs, "os", data.os);
            insert_workflow_text(&mut outputs, "architecture", data.architecture);
            if let Some(hostname) = data.hostname {
                insert_workflow_text(&mut outputs, "hostname", hostname);
            }
            if let Some(uptime_seconds) = data.uptime_seconds {
                insert_workflow_u128(&mut outputs, "uptime_seconds", u128::from(uptime_seconds));
            }
        }
        OutputData::Cpu(data) => {
            insert_workflow_usize(&mut outputs, "logical_cores", data.logical_cores);
            if let Some(physical_cores) = data.physical_cores {
                insert_workflow_usize(&mut outputs, "physical_cores", physical_cores);
            }
            if let Some(frequency_mhz) = data.frequency_mhz {
                insert_workflow_u128(&mut outputs, "frequency_mhz", u128::from(frequency_mhz));
            }
        }
        OutputData::Memory(data) => {
            insert_workflow_u128(&mut outputs, "total_bytes", u128::from(data.total_bytes));
            insert_workflow_u128(&mut outputs, "used_bytes", u128::from(data.used_bytes));
            insert_workflow_u128(
                &mut outputs,
                "available_bytes",
                u128::from(data.available_bytes),
            );
        }
        OutputData::Disk(data) => {
            insert_workflow_usize(&mut outputs, "volume_count", data.volumes.len());
        }
        OutputData::Sha256(data) => {
            insert_workflow_text(&mut outputs, "sha256", data.sha256);
            insert_workflow_u128(&mut outputs, "size_bytes", u128::from(data.size_bytes));
        }
        OutputData::WebDiagnostics(data) => {
            insert_workflow_text(&mut outputs, "hostname", data.hostname);
            insert_workflow_integer(&mut outputs, "port", i64::from(data.port));
        }
        OutputData::Interfaces(_) => {}
    }
    outputs
}

fn insert_workflow_text(outputs: &mut BTreeMap<String, WorkflowValue>, name: &str, value: String) {
    if value.len() <= nyra::workflow_definition::MAX_TEXT_VALUE_BYTES && !value.contains('\0') {
        outputs.insert(name.to_owned(), WorkflowValue::Text(value));
    }
}

fn insert_workflow_integer(outputs: &mut BTreeMap<String, WorkflowValue>, name: &str, value: i64) {
    outputs.insert(name.to_owned(), WorkflowValue::Integer(value));
}

fn insert_workflow_u128(outputs: &mut BTreeMap<String, WorkflowValue>, name: &str, value: u128) {
    if let Ok(value) = i64::try_from(value) {
        insert_workflow_integer(outputs, name, value);
    }
}

fn insert_workflow_usize(outputs: &mut BTreeMap<String, WorkflowValue>, name: &str, value: usize) {
    if let Ok(value) = i64::try_from(value) {
        insert_workflow_integer(outputs, name, value);
    }
}

fn workflow_exit_code(result: &WorkflowRunResult) -> u8 {
    match result.execution_status {
        WorkflowExecutionStatus::Failed => 3,
        WorkflowExecutionStatus::TimedOut => 4,
        WorkflowExecutionStatus::Cancelled => 130,
        WorkflowExecutionStatus::Completed => {
            if result.steps.iter().any(|step| {
                matches!(
                    step.finding,
                    Some(
                        WorkflowFinding::Refused
                            | WorkflowFinding::TimedOut
                            | WorkflowFinding::Unreachable
                            | WorkflowFinding::Inconclusive
                            | WorkflowFinding::Unhealthy
                    )
                )
            }) {
                1
            } else {
                0
            }
        }
    }
}

fn write_workflow_result(stdout: &mut impl Write, result: &WorkflowRunResult) -> io::Result<()> {
    writeln!(
        stdout,
        "Workflow {}: {:?} ({} ms)",
        result.workflow_id, result.execution_status, result.duration_ms
    )?;
    for step in &result.steps {
        match step.finding {
            Some(finding) => writeln!(
                stdout,
                "{}: {:?}, {:?} ({} ms)",
                step.id, step.status, finding, step.duration_ms
            )?,
            None => writeln!(
                stdout,
                "{}: {:?} ({} ms)",
                step.id, step.status, step.duration_ms
            )?,
        }
        if let Some(issue) = &step.issue {
            writeln!(stdout, "  Issue: {}", workflow_issue_label(issue))?;
        }
    }
    Ok(())
}

fn workflow_issue_label(issue: &WorkflowRunIssue) -> &'static str {
    match issue {
        WorkflowRunIssue::ConditionNotMet => "condition not met",
        WorkflowRunIssue::StepTimedOut => "step timed out",
        WorkflowRunIssue::WorkflowTimedOut => "workflow time budget expired",
        WorkflowRunIssue::MissingInput { .. } => "required input is missing",
        WorkflowRunIssue::MissingStepOutput { .. } => "referenced step output is unavailable",
        WorkflowRunIssue::InvalidToolOutput => "tool returned an invalid result",
        WorkflowRunIssue::ToolFailed => "tool execution failed",
        WorkflowRunIssue::Cancelled => "cancelled",
    }
}

fn run_theme(stdout: &mut impl Write, command: ThemeCommand) -> io::Result<u8> {
    let result = match command {
        ThemeCommand::Validate { path } => validate_custom_theme_file(&path)
            .map(|name| format!("Valid custom theme: {name:?} (version 1).")),
        ThemeCommand::Import { source, force } => import_custom_theme_file(&source, force)
            .map(|_| "Custom theme installed in NYRA's active theme directory.".to_owned()),
        ThemeCommand::Export { destination, force } => {
            export_custom_theme_file(&destination, force)
                .map(|_| "Custom theme exported.".to_owned())
        }
    };

    match result {
        Ok(message) => {
            writeln!(stdout, "{message}")?;
            Ok(0)
        }
        Err(error) => {
            writeln!(io::stderr().lock(), "Error: {error}")?;
            Ok(theme_error_exit_code(error))
        }
    }
}

const fn theme_error_exit_code(error: CustomThemeFileError) -> u8 {
    error.exit_code()
}

fn run_knowledge(stdout: &mut impl Write, command: KnowledgeCommand) -> io::Result<u8> {
    match command {
        KnowledgeCommand::List => {
            writeln!(stdout, "ID\tOS\tTITLE\tRISK")?;
            for card in KNOWLEDGE_CARDS {
                writeln!(
                    stdout,
                    "{}\t{}\t{}\t{}",
                    card.id, card.operating_system, card.title, card.risk
                )?;
            }
            Ok(0)
        }
        KnowledgeCommand::Search { query } => {
            let query = query.join(" ");
            let matches = search_knowledge_cards(&query);
            if matches.is_empty() {
                writeln!(stdout, "No knowledge cards match that query.")?;
                return Ok(0);
            }
            writeln!(stdout, "ID\tOS\tTITLE\tRISK")?;
            for card in matches {
                writeln!(
                    stdout,
                    "{}\t{}\t{}\t{}",
                    card.id, card.operating_system, card.title, card.risk
                )?;
            }
            Ok(0)
        }
        KnowledgeCommand::Show { card_id } => {
            let Some(card) = find_knowledge_card(&card_id) else {
                writeln!(
                    io::stderr().lock(),
                    "Error: unknown knowledge card; run `nyra knowledge list` to see available IDs."
                )?;
                return Ok(2);
            };
            writeln!(stdout, "{} — {}", card.title, card.id)?;
            writeln!(stdout, "Operating system: {}", card.operating_system)?;
            writeln!(stdout, "Intent: {}", card.intent)?;
            writeln!(stdout, "Command: {}", card.command)?;
            writeln!(stdout, "Parameters: {}", card.parameters)?;
            writeln!(stdout, "Example: {}", card.example)?;
            writeln!(stdout, "Expected output: {}", card.expected_output)?;
            writeln!(stdout, "Explanation: {}", card.explanation)?;
            writeln!(stdout, "Risk: {}", card.risk)?;
            writeln!(stdout, "Side effects: {}", card.side_effects)?;
            writeln!(stdout, "Required privileges: {}", card.required_privileges)?;
            writeln!(
                stdout,
                "Related diagnostics: {}",
                card.related_diagnostics.join(", ")
            )?;
            writeln!(
                stdout,
                "NYRA displays this reference and never runs the command."
            )?;
            Ok(0)
        }
    }
}

fn parsed_check(
    stdout: &mut impl Write,
    tool_id: &str,
    fields: Vec<String>,
    output: Output,
    report_options: ReportOptions,
) -> io::Result<u8> {
    let input = match parse_tool_input(tool_id, &fields) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("Error: {}", error.message);
            return Ok(2);
        }
    };
    let report_destination = match prepare_report_destination(report_options) {
        Ok(destination) => destination,
        Err(error) => {
            eprintln!("Error: {error}");
            return Ok(error.exit_code());
        }
    };
    run_check_with_report(stdout, input, output, report_destination)
}

fn run_check(
    stdout: &mut impl Write,
    input: ToolInput,
    output: Output,
    report_options: ReportOptions,
) -> io::Result<u8> {
    let report_destination = match prepare_report_destination(report_options) {
        Ok(destination) => destination,
        Err(error) => {
            eprintln!("Error: {error}");
            return Ok(error.exit_code());
        }
    };
    run_check_with_report(stdout, input, output, report_destination)
}

fn prepare_report_destination(
    options: ReportOptions,
) -> Result<Option<ReportDestination>, ReportExportError> {
    match (options.report_json, options.report_html, options.report_csv) {
        (Some(path), None, None) => JsonReportDestination::prepare(&path)
            .map(ReportDestination::Json)
            .map(Some),
        (None, Some(path), None) => HtmlReportDestination::prepare(&path)
            .map(ReportDestination::Html)
            .map(Some),
        (None, None, Some(path)) => CsvReportDestination::prepare(&path)
            .map(ReportDestination::Csv)
            .map(Some),
        (None, None, None) => Ok(None),
        _ => unreachable!("Clap enforces that only one report format is selected"),
    }
}

fn run_workflow_check(
    stdout: &mut impl Write,
    target: String,
    timeout: u64,
    output: Output,
    report_json: Option<PathBuf>,
    report_html: Option<PathBuf>,
    report_csv: Option<PathBuf>,
) -> io::Result<u8> {
    let input = match parse_tool_input("workflow.web_diagnostics", &[target, timeout.to_string()]) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("Error: {}", error.message);
            return Ok(2);
        }
    };
    let destination = match prepare_report_destination(ReportOptions {
        report_json,
        report_html,
        report_csv,
    }) {
        Ok(destination) => destination,
        Err(error) => {
            eprintln!("Error: {error}");
            return Ok(error.exit_code());
        }
    };
    run_check_with_report(stdout, input, output, destination)
}

fn run_check_with_report(
    stdout: &mut impl Write,
    input: ToolInput,
    output: Output,
    report_destination: Option<ReportDestination>,
) -> io::Result<u8> {
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
    let report_result = report_destination.map(|destination| {
        let success_message = destination.success_message();
        (success_message, destination.write(&result))
    });
    match output {
        Output::Json => {
            serde_json::to_writer(&mut *stdout, &result)?;
            writeln!(stdout)?;
        }
        Output::Text => {
            if let Some(observer) = result.observer.as_deref() {
                writeln!(stdout, "Observed from: {observer}")?;
            }
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
                        if info.volumes.is_empty() {
                            writeln!(stdout, "Disk measurements unavailable")?;
                        }
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
                    OutputData::WebDiagnostics(info) => {
                        writeln!(stdout, "Summary: {}", info.summary)?;
                        writeln!(stdout, "URL: {}", info.url)?;
                        if let Some(address) = info.selected_address {
                            writeln!(stdout, "Selected peer: {address}")?;
                        }
                        for step in info.steps {
                            let status = match step.status {
                                nyra::WorkflowStepStatus::Completed => "completed",
                                nyra::WorkflowStepStatus::Skipped => "skipped",
                                nyra::WorkflowStepStatus::Failed => "failed",
                                nyra::WorkflowStepStatus::Cancelled => "cancelled",
                            };
                            let finding = step
                                .finding
                                .map_or_else(String::new, |value| format!(" · {value}"));
                            writeln!(
                                stdout,
                                "{}: {status}{finding} ({} ms)",
                                step.id, step.duration_ms
                            )?;
                            if let Some(reason) = step.reason {
                                writeln!(stdout, "  {reason}")?;
                            }
                            if let Some(data) = step.data {
                                writeln!(stdout, "  {data}")?;
                            }
                        }
                        if !info.next_checks.is_empty() {
                            writeln!(stdout, "Next checks:")?;
                            for check in info.next_checks {
                                writeln!(stdout, "  {check}")?;
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(error) = result.error {
        eprintln!("Error: {error}");
    }
    stdout.flush()?;
    if let Some((success_message, report_result)) = report_result {
        match report_result {
            Ok(()) => eprintln!("{success_message}"),
            Err(error) => {
                eprintln!("Error: {error}");
                return Ok(error.exit_code());
            }
        }
    }
    Ok(exit_code)
}
