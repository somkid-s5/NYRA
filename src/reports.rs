use crate::{
    Finding, OutputData, RunError, RunResult, WorkflowStep, WorkflowStepStatus,
    workflow_definition::{WorkflowFinding, WorkflowStepStatus as CustomWorkflowStepStatus},
    workflow_runner::{WorkflowExecutionStatus, WorkflowRunIssue, WorkflowRunResult},
};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const REPORT_SCHEMA_VERSION: u8 = 1;
const MAX_REPORT_BYTES: usize = 64 * 1024;
static REPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A prepared destination for a new, sanitized JSON diagnostic report.
///
/// Preparation checks the destination before a diagnostic starts and creates a
/// temporary file beside it. The final path is created only after the report
/// has been fully serialized, and existing files are never replaced.
pub struct JsonReportDestination {
    destination: PreparedReportFile,
}

/// A prepared destination for a new, sanitized HTML report.
///
/// HTML reports include only common execution metadata and, for Web
/// Diagnostics, its static summary and sanitized workflow steps. Targets and
/// raw tool data are always omitted.
pub struct HtmlReportDestination {
    destination: PreparedReportFile,
}

/// A prepared destination for a new, sanitized CSV report.
///
/// The report uses a stable `section,key,value,duration_ms,status,status_code`
/// layout and never serializes raw tool output.
pub struct CsvReportDestination {
    destination: PreparedReportFile,
}

struct PreparedReportFile {
    path: PathBuf,
    temporary: NamedTempFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportExportError {
    InvalidDestination,
    DestinationExists,
    DirectoryUnavailable,
    UnsupportedResult,
    SerializationFailed,
    TooLarge,
    WriteFailed,
}

impl ReportExportError {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvalidDestination | Self::DestinationExists => 2,
            Self::DirectoryUnavailable
            | Self::UnsupportedResult
            | Self::SerializationFailed
            | Self::TooLarge
            | Self::WriteFailed => 3,
        }
    }
}

impl std::fmt::Display for ReportExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidDestination => "report destination must identify a file",
            Self::DestinationExists => {
                "report destination already exists; NYRA will not overwrite it"
            }
            Self::DirectoryUnavailable => "report directory is unavailable or not writable",
            Self::UnsupportedResult => "the selected report format does not support this result",
            Self::SerializationFailed => "NYRA could not encode the report",
            Self::TooLarge => "report exceeds the 64 KiB export limit",
            Self::WriteFailed => "NYRA could not finish writing the report",
        })
    }
}

impl std::error::Error for ReportExportError {}

impl JsonReportDestination {
    pub fn prepare(path: &Path) -> Result<Self, ReportExportError> {
        Ok(Self {
            destination: PreparedReportFile::prepare(path)?,
        })
    }

    pub fn write(self, result: &RunResult) -> Result<(), ReportExportError> {
        let report = SanitizedWorkflowReport::from_result(result)?;
        let mut bytes =
            serde_json::to_vec(&report).map_err(|_| ReportExportError::SerializationFailed)?;
        bytes.push(b'\n');
        self.destination.write(&bytes)
    }

    /// Write a sanitized report for a user-defined workflow execution.
    pub fn write_workflow_run(self, result: &WorkflowRunResult) -> Result<(), ReportExportError> {
        let report = SanitizedWorkflowRunReport::from_result(result)?;
        let mut bytes =
            serde_json::to_vec(&report).map_err(|_| ReportExportError::SerializationFailed)?;
        bytes.push(b'\n');
        self.destination.write(&bytes)
    }
}

impl HtmlReportDestination {
    /// Prepare a new HTML report destination without replacing existing files.
    pub fn prepare(path: &Path) -> Result<Self, ReportExportError> {
        Ok(Self {
            destination: PreparedReportFile::prepare(path)?,
        })
    }

    /// Write a sanitized report for any completed or failed tool result.
    pub fn write(self, result: &RunResult) -> Result<(), ReportExportError> {
        let report = SanitizedTextReport::from_result(result)?;
        let bytes = render_html_report(&report).into_bytes();
        self.destination.write(&bytes)
    }

    /// Write a sanitized report for a user-defined workflow execution.
    pub fn write_workflow_run(self, result: &WorkflowRunResult) -> Result<(), ReportExportError> {
        let report = SanitizedTextReport::from_workflow_run(result)?;
        let bytes = render_html_report(&report).into_bytes();
        self.destination.write(&bytes)
    }
}

impl CsvReportDestination {
    /// Prepare a new CSV report destination without replacing existing files.
    pub fn prepare(path: &Path) -> Result<Self, ReportExportError> {
        Ok(Self {
            destination: PreparedReportFile::prepare(path)?,
        })
    }

    /// Write a sanitized report for any completed or failed tool result.
    pub fn write(self, result: &RunResult) -> Result<(), ReportExportError> {
        let report = SanitizedTextReport::from_result(result)?;
        let bytes = render_csv_report(&report).into_bytes();
        self.destination.write(&bytes)
    }

    /// Write a sanitized report for a user-defined workflow execution.
    pub fn write_workflow_run(self, result: &WorkflowRunResult) -> Result<(), ReportExportError> {
        let report = SanitizedTextReport::from_workflow_run(result)?;
        let bytes = render_csv_report(&report).into_bytes();
        self.destination.write(&bytes)
    }
}

impl PreparedReportFile {
    fn prepare(path: &Path) -> Result<Self, ReportExportError> {
        if path.as_os_str().is_empty() || path.file_name().is_none() {
            return Err(ReportExportError::InvalidDestination);
        }
        match fs::symlink_metadata(path) {
            Ok(_) => return Err(ReportExportError::DestinationExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ReportExportError::DirectoryUnavailable),
        }

        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let temporary =
            NamedTempFile::new_in(parent).map_err(|_| ReportExportError::DirectoryUnavailable)?;
        Ok(Self {
            path: path.to_owned(),
            temporary,
        })
    }

    fn write(mut self, bytes: &[u8]) -> Result<(), ReportExportError> {
        if bytes.len() > MAX_REPORT_BYTES {
            return Err(ReportExportError::TooLarge);
        }
        self.temporary
            .write_all(bytes)
            .map_err(|_| ReportExportError::WriteFailed)?;
        self.temporary
            .as_file()
            .sync_all()
            .map_err(|_| ReportExportError::WriteFailed)?;
        let path = self.path.clone();
        self.temporary.persist_noclobber(path).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                ReportExportError::DestinationExists
            } else {
                ReportExportError::WriteFailed
            }
        })?;
        Ok(())
    }
}

struct SanitizedTextReport {
    report_id: String,
    exported_at_utc: String,
    platform: String,
    tool_id: &'static str,
    custom_workflow: bool,
    execution_status: &'static str,
    finding: Option<String>,
    duration_ms: u128,
    error_code: Option<&'static str>,
    summary: Option<String>,
    next_checks: Vec<String>,
    steps: Vec<SanitizedTextStep>,
}

struct SanitizedTextStep {
    id: String,
    status: &'static str,
    finding: Option<String>,
    duration_ms: u128,
    reason: Option<String>,
    status_code: Option<u16>,
}

impl SanitizedTextReport {
    fn from_result(result: &RunResult) -> Result<Self, ReportExportError> {
        let exported_at = OffsetDateTime::now_utc();
        let exported_at_utc = exported_at
            .format(&Rfc3339)
            .map_err(|_| ReportExportError::SerializationFailed)?;
        let sequence = REPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let report_id = format!(
            "{}-{}-{sequence}",
            exported_at.unix_timestamp_nanos(),
            std::process::id()
        );
        let (summary, next_checks, steps) = match result.data.as_ref() {
            Some(OutputData::WebDiagnostics(data)) => (
                Some(safe_summary(result.error, result.finding, &data.steps)),
                safe_next_checks(result.finding, &data.steps)
                    .iter()
                    .map(|check| (*check).to_owned())
                    .collect(),
                data.steps.iter().filter_map(sanitize_text_step).collect(),
            ),
            _ => (None, Vec::new(), Vec::new()),
        };

        Ok(Self {
            report_id,
            exported_at_utc,
            platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            tool_id: safe_tool_id(result.tool_id),
            custom_workflow: false,
            execution_status: result.execution_status_label(),
            finding: result.finding.map(|finding| finding.to_string()),
            duration_ms: result.duration_ms,
            error_code: result.error.map(stable_error_code),
            summary,
            next_checks,
            steps,
        })
    }

    fn from_workflow_run(result: &WorkflowRunResult) -> Result<Self, ReportExportError> {
        let report = SanitizedWorkflowRunReport::from_result(result)?;
        Ok(Self {
            report_id: report.report_id,
            exported_at_utc: report.exported_at_utc,
            platform: report.platform,
            tool_id: report.workflow_id.display,
            custom_workflow: true,
            execution_status: match report.execution_status {
                WorkflowExecutionStatus::Completed => "completed",
                WorkflowExecutionStatus::Failed => "failed",
                WorkflowExecutionStatus::TimedOut => "timed_out",
                WorkflowExecutionStatus::Cancelled => "cancelled",
            },
            finding: None,
            duration_ms: report.duration_ms,
            error_code: None,
            summary: None,
            next_checks: Vec::new(),
            steps: report
                .steps
                .into_iter()
                .map(|step| SanitizedTextStep {
                    id: format!("{} {}", step.step_index, step.step_id.display),
                    status: match step.status {
                        CustomWorkflowStepStatus::Completed => "completed",
                        CustomWorkflowStepStatus::Skipped => "skipped",
                        CustomWorkflowStepStatus::Failed => "failed",
                        CustomWorkflowStepStatus::Cancelled => "cancelled",
                    },
                    finding: step.finding.map(|finding| format!("{finding:?}")),
                    duration_ms: step.duration_ms,
                    reason: step.issue_summary.map(str::to_owned),
                    status_code: None,
                })
                .collect(),
        })
    }
}

fn sanitize_text_step(step: &WorkflowStep) -> Option<SanitizedTextStep> {
    // Workflow IDs are fixed by the compiled-in Web Diagnostics workflow.
    // Ignore unknown IDs so this report never turns arbitrary data into a label.
    let id = match step.id {
        "dns" => "dns",
        "tcp" => "tcp",
        "tls" => "tls",
        "http" => "http",
        _ => return None,
    };
    let status = match step.status {
        WorkflowStepStatus::Completed => "completed",
        WorkflowStepStatus::Skipped => "skipped",
        WorkflowStepStatus::Failed => "failed",
        WorkflowStepStatus::Cancelled => "cancelled",
    };
    let status_code = if id == "http" {
        step.data
            .as_ref()
            .and_then(|data| data.get("status_code"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|code| u16::try_from(code).ok())
    } else {
        None
    };
    Some(SanitizedTextStep {
        id: id.to_owned(),
        status,
        finding: step.finding.map(|finding| finding.to_string()),
        duration_ms: step.duration_ms,
        reason: step
            .reason
            .as_ref()
            .map(|_| safe_step_reason(step.status).to_owned()),
        status_code,
    })
}

fn render_html_report(report: &SanitizedTextReport) -> String {
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>NYRA Diagnostic Report</title><style>body{font:16px system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem;color:#18212b}table{border-collapse:collapse;width:100%;margin:1rem 0}th,td{border:1px solid #cbd5e1;padding:.5rem;text-align:left;vertical-align:top}th{background:#f1f5f9}code{overflow-wrap:anywhere}</style></head><body><h1>NYRA Diagnostic Report</h1><table><thead><tr><th>Field</th><th>Value</th></tr></thead><tbody>",
    );
    html_row(&mut html, "Report ID", &report.report_id);
    html_row(&mut html, "Exported at (UTC)", &report.exported_at_utc);
    html_row(&mut html, "Platform", &report.platform);
    html_row(
        &mut html,
        if report.custom_workflow {
            "Workflow"
        } else {
            "Tool"
        },
        report.tool_id,
    );
    html_row(&mut html, "Execution status", report.execution_status);
    html_row(
        &mut html,
        "Finding",
        report.finding.as_deref().unwrap_or("not available"),
    );
    html_row(&mut html, "Duration (ms)", &report.duration_ms.to_string());
    html_row(&mut html, "Target", "[redacted]");
    if let Some(error_code) = report.error_code {
        html_row(&mut html, "Error code", error_code);
    }
    if let Some(summary) = &report.summary {
        html_row(&mut html, "Summary", summary);
    }
    html.push_str("</tbody></table>");
    if !report.next_checks.is_empty() {
        html.push_str("<h2>Next checks</h2><ul>");
        for check in &report.next_checks {
            html.push_str("<li>");
            html.push_str(&escape_html(check));
            html.push_str("</li>");
        }
        html.push_str("</ul>");
    }
    if !report.steps.is_empty() {
        html.push_str("<h2>Workflow steps</h2><table><thead><tr><th>ID</th><th>Status</th><th>Finding</th><th>Duration (ms)</th><th>Reason</th><th>HTTP status</th></tr></thead><tbody>");
        for step in &report.steps {
            html.push_str("<tr><td>");
            html.push_str(&escape_html(&step.id));
            html.push_str("</td><td>");
            html.push_str(&escape_html(step.status));
            html.push_str("</td><td>");
            html.push_str(&escape_html(step.finding.as_deref().unwrap_or("")));
            html.push_str("</td><td>");
            html.push_str(&step.duration_ms.to_string());
            html.push_str("</td><td>");
            html.push_str(&escape_html(step.reason.as_deref().unwrap_or("")));
            html.push_str("</td><td>");
            html.push_str(
                &step
                    .status_code
                    .map_or_else(String::new, |code| code.to_string()),
            );
            html.push_str("</td></tr>");
        }
        html.push_str("</tbody></table>");
    }
    html.push_str("</body></html>\n");
    html
}

fn html_row(html: &mut String, label: &str, value: &str) {
    html.push_str("<tr><th>");
    html.push_str(&escape_html(label));
    html.push_str("</th><td>");
    html.push_str(&escape_html(value));
    html.push_str("</td></tr>");
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn render_csv_report(report: &SanitizedTextReport) -> String {
    let mut csv = String::from("section,key,value,duration_ms,status,status_code\r\n");
    csv_row(
        &mut csv,
        ["metadata", "report_id", &report.report_id, "", "", ""],
    );
    csv_row(
        &mut csv,
        [
            "metadata",
            "exported_at_utc",
            &report.exported_at_utc,
            "",
            "",
            "",
        ],
    );
    csv_row(
        &mut csv,
        ["metadata", "platform", &report.platform, "", "", ""],
    );
    csv_row(
        &mut csv,
        [
            "metadata",
            if report.custom_workflow {
                "workflow_id"
            } else {
                "tool_id"
            },
            report.tool_id,
            "",
            "",
            "",
        ],
    );
    csv_row(
        &mut csv,
        [
            "metadata",
            "execution_status",
            report.execution_status,
            "",
            "",
            "",
        ],
    );
    csv_row(
        &mut csv,
        [
            "metadata",
            "finding",
            report.finding.as_deref().unwrap_or("not available"),
            "",
            "",
            "",
        ],
    );
    csv_row(
        &mut csv,
        [
            "metadata",
            "duration_ms",
            &report.duration_ms.to_string(),
            "",
            "",
            "",
        ],
    );
    csv_row(&mut csv, ["metadata", "target", "[redacted]", "", "", ""]);
    if let Some(error_code) = report.error_code {
        csv_row(&mut csv, ["metadata", "error_code", error_code, "", "", ""]);
    }
    if let Some(summary) = &report.summary {
        csv_row(&mut csv, ["workflow", "summary", summary, "", "", ""]);
    }
    for check in &report.next_checks {
        csv_row(&mut csv, ["next_check", "", check, "", "", ""]);
    }
    for step in &report.steps {
        let duration = step.duration_ms.to_string();
        let status_code = step
            .status_code
            .map_or_else(String::new, |code| code.to_string());
        csv_row(
            &mut csv,
            [
                "step",
                &step.id,
                step.finding.as_deref().unwrap_or(""),
                &duration,
                step.status,
                &status_code,
            ],
        );
        if let Some(reason) = &step.reason {
            csv_row(&mut csv, ["step_reason", &step.id, reason, "", "", ""]);
        }
    }
    csv
}

fn csv_row<const N: usize>(csv: &mut String, cells: [&str; N]) {
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            csv.push(',');
        }
        let first_significant = cell.chars().find(|character| {
            !character.is_whitespace() && !character.is_control() && *character != '\u{feff}'
        });
        let formula_safe = if matches!(
            first_significant,
            Some('=' | '+' | '-' | '@' | '＝' | '＋' | '－' | '＠')
        ) {
            format!("'{cell}")
        } else {
            (*cell).to_owned()
        };
        csv.push('"');
        for character in formula_safe.chars() {
            if character == '"' {
                csv.push_str("\"\"");
            } else {
                csv.push(character);
            }
        }
        csv.push('"');
    }
    csv.push_str("\r\n");
}

#[derive(Serialize)]
struct SanitizedWorkflowReport {
    report_schema_version: u8,
    report_id: String,
    app_version: &'static str,
    exported_at_utc: String,
    platform: String,
    tool_id: &'static str,
    execution_status: &'static str,
    finding: Option<Finding>,
    duration_ms: u128,
    target: ReportTarget,
    summary: String,
    next_checks: Vec<String>,
    steps: Vec<SanitizedWorkflowStep>,
    error_code: Option<&'static str>,
}

#[derive(Serialize)]
struct ReportTarget {
    display: &'static str,
    redacted: bool,
}

#[derive(Serialize)]
struct SanitizedWorkflowStep {
    id: &'static str,
    status: WorkflowStepStatus,
    finding: Option<Finding>,
    duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_code: Option<u16>,
}

#[derive(Serialize)]
struct SanitizedWorkflowRunReport {
    report_schema_version: u8,
    report_id: String,
    app_version: &'static str,
    exported_at_utc: String,
    platform: String,
    workflow_result_schema_version: u8,
    workflow_id: ReportTarget,
    execution_status: WorkflowExecutionStatus,
    duration_ms: u128,
    steps: Vec<SanitizedWorkflowRunStep>,
}

#[derive(Serialize)]
struct SanitizedWorkflowRunStep {
    step_index: usize,
    step_id: ReportTarget,
    status: CustomWorkflowStepStatus,
    finding: Option<WorkflowFinding>,
    duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    issue_summary: Option<&'static str>,
}

impl SanitizedWorkflowRunReport {
    fn from_result(result: &WorkflowRunResult) -> Result<Self, ReportExportError> {
        if result.steps.len() > crate::workflow_definition::MAX_STEPS {
            return Err(ReportExportError::TooLarge);
        }
        let exported_at = OffsetDateTime::now_utc();
        let exported_at_utc = exported_at
            .format(&Rfc3339)
            .map_err(|_| ReportExportError::SerializationFailed)?;
        let sequence = REPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let report_id = format!(
            "{}-{}-{sequence}",
            exported_at.unix_timestamp_nanos(),
            std::process::id()
        );
        let redacted_identifier = || ReportTarget {
            display: "[redacted]",
            redacted: true,
        };

        Ok(Self {
            report_schema_version: REPORT_SCHEMA_VERSION,
            report_id,
            app_version: env!("CARGO_PKG_VERSION"),
            exported_at_utc,
            platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            workflow_result_schema_version: result.schema_version,
            workflow_id: redacted_identifier(),
            execution_status: result.execution_status,
            duration_ms: result.duration_ms,
            steps: result
                .steps
                .iter()
                .enumerate()
                .map(|(index, step)| SanitizedWorkflowRunStep {
                    step_index: index + 1,
                    step_id: redacted_identifier(),
                    status: step.status,
                    finding: step.finding,
                    duration_ms: step.duration_ms,
                    issue_summary: step.issue.as_ref().map(workflow_issue_summary),
                })
                .collect(),
        })
    }
}

fn workflow_issue_summary(issue: &WorkflowRunIssue) -> &'static str {
    match issue {
        WorkflowRunIssue::ConditionNotMet => "Condition not met.",
        WorkflowRunIssue::StepTimedOut => "Step time budget expired.",
        WorkflowRunIssue::WorkflowTimedOut => "Workflow time budget expired.",
        WorkflowRunIssue::MissingInput { .. } => "Required input is unavailable.",
        WorkflowRunIssue::MissingStepOutput { .. } => "Required step output is unavailable.",
        WorkflowRunIssue::InvalidToolOutput => "Tool returned an invalid result.",
        WorkflowRunIssue::ToolFailed => "Tool execution failed.",
        WorkflowRunIssue::Cancelled => "Run was cancelled.",
    }
}

impl SanitizedWorkflowReport {
    fn from_result(result: &RunResult) -> Result<Self, ReportExportError> {
        let exported_at = OffsetDateTime::now_utc();
        let exported_at_utc = exported_at
            .format(&Rfc3339)
            .map_err(|_| ReportExportError::SerializationFailed)?;
        let sequence = REPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let report_id = format!(
            "{}-{}-{sequence}",
            exported_at.unix_timestamp_nanos(),
            std::process::id()
        );
        let (summary, next_checks, steps) = match result.data.as_ref() {
            Some(OutputData::WebDiagnostics(data)) => (
                safe_summary(result.error, result.finding, &data.steps),
                safe_next_checks(result.finding, &data.steps)
                    .iter()
                    .map(|check| (*check).to_owned())
                    .collect(),
                data.steps.iter().filter_map(sanitize_step).collect(),
            ),
            _ => (safe_tool_summary(result), Vec::new(), Vec::new()),
        };

        Ok(Self {
            report_schema_version: REPORT_SCHEMA_VERSION,
            report_id,
            app_version: env!("CARGO_PKG_VERSION"),
            exported_at_utc,
            platform: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            tool_id: safe_tool_id(result.tool_id),
            execution_status: result.execution_status_label(),
            finding: result.finding,
            duration_ms: result.duration_ms,
            target: ReportTarget {
                display: "[redacted]",
                redacted: true,
            },
            summary,
            next_checks,
            steps,
            error_code: result.error.map(stable_error_code),
        })
    }
}

fn safe_tool_summary(result: &RunResult) -> String {
    let tool_name = crate::find_tool(result.tool_id).map_or("Diagnostic", |tool| tool.name);
    match (result.error, result.finding) {
        (Some(error), _) => format!(
            "{tool_name} could not complete ({})",
            stable_error_code(error)
        ),
        (None, Some(finding)) => format!("{tool_name} reported {finding}."),
        (None, None) => format!("{tool_name} completed without a diagnostic finding."),
    }
}

fn sanitize_step(step: &WorkflowStep) -> Option<SanitizedWorkflowStep> {
    let id = report_step_id(step.id)?;
    let status_code = if id == "http" {
        step.data
            .as_ref()
            .and_then(|data| data.get("status_code"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|code| u16::try_from(code).ok())
    } else {
        None
    };
    Some(SanitizedWorkflowStep {
        id,
        status: step.status,
        finding: step.finding,
        duration_ms: step.duration_ms,
        reason: step.reason.as_ref().map(|_| safe_step_reason(step.status)),
        status_code,
    })
}

fn safe_tool_id(tool_id: &str) -> &'static str {
    crate::find_tool(tool_id).map_or("unknown", |tool| tool.id)
}

fn report_step_id(step_id: &str) -> Option<&'static str> {
    match step_id {
        "dns" => Some("dns"),
        "tcp" => Some("tcp"),
        "tls" => Some("tls"),
        "http" => Some("http"),
        _ => None,
    }
}

fn safe_step_reason(status: WorkflowStepStatus) -> &'static str {
    match status {
        WorkflowStepStatus::Completed => "Step completed; see its finding.",
        WorkflowStepStatus::Skipped => "Step was skipped because a prerequisite did not pass.",
        WorkflowStepStatus::Failed => "Step could not complete.",
        WorkflowStepStatus::Cancelled => "Step was cancelled.",
    }
}

fn is_blocking_finding(finding: Option<Finding>) -> bool {
    matches!(
        finding,
        Some(
            Finding::Refused
                | Finding::TimedOut
                | Finding::Unreachable
                | Finding::Inconclusive
                | Finding::Unhealthy
        )
    )
}

fn blocking_step_id(steps: &[WorkflowStep]) -> Option<&'static str> {
    steps
        .iter()
        .rev()
        .find(|step| is_blocking_finding(step.finding))
        .and_then(|step| report_step_id(step.id))
}

fn safe_summary(
    error: Option<RunError>,
    finding: Option<Finding>,
    steps: &[WorkflowStep],
) -> String {
    match (error, finding) {
        (Some(RunError::Cancelled), _) => {
            "Workflow cancelled; completed observations are preserved.".to_owned()
        }
        (Some(_), _) => "A workflow step could not execute; see the step details.".to_owned(),
        (None, Some(Finding::Healthy)) => {
            "All applicable protocol layers completed successfully.".to_owned()
        }
        (None, Some(value)) => blocking_step_id(steps).map_or_else(
            || format!("The workflow completed with {value}."),
            |step_id| format!("The {step_id} step reported {value}."),
        ),
        (None, None) => "The workflow did not produce a final protocol finding.".to_owned(),
    }
}

fn safe_next_checks(finding: Option<Finding>, steps: &[WorkflowStep]) -> &'static [&'static str] {
    match (finding, blocking_step_id(steps)) {
        (Some(Finding::Healthy), _) => &[],
        (Some(Finding::TimedOut), _) => {
            &["Compare from another authorized observer or repeat with a longer timeout."]
        }
        (Some(Finding::Unreachable), Some("dns")) => &[
            "Verify the hostname and system resolver configuration; this finding is limited to name resolution.",
        ],
        (Some(Finding::Unreachable), Some("http")) => &[
            "Check whether the selected peer is accepting HTTP after its TCP connection succeeded.",
        ],
        (Some(Finding::Unreachable), _) => {
            &["Check routing and firewall policy from this observer."]
        }
        (Some(Finding::Refused), _) => &[
            "Check whether the service is listening on the selected peer and port.",
            "Review firewall rules between this observer and the peer.",
        ],
        (Some(Finding::Unhealthy), Some("tls")) => &[
            "Review certificate validity, hostname coverage, and the system trust chain.",
            "HTTP was withheld because certificate verification failed.",
        ],
        (Some(Finding::Unhealthy), Some("http")) => &[
            "Inspect the reported HTTP status and selected headers.",
            "Follow a reported redirect only as a separate explicit check.",
        ],
        _ => &["Review the completed step findings and their reasons."],
    }
}

const fn stable_error_code(error: RunError) -> &'static str {
    match error {
        RunError::ResolutionFailed => "resolution_failed",
        RunError::SocketFailed => "socket_failed",
        RunError::DeadlineExceeded => "deadline_exceeded",
        RunError::Cancelled => "cancelled",
        RunError::SignalUnavailable => "signal_unavailable",
        RunError::SystemInfoUnavailable => "system_info_unavailable",
        RunError::FileOpenFailed => "file_open_failed",
        RunError::FileReadFailed => "file_read_failed",
        RunError::ResolverUnavailable => "resolver_unavailable",
        RunError::InvalidInput => "invalid_input",
        RunError::HttpClientFailed => "http_client_failed",
        RunError::NetworkRequestFailed => "network_request_failed",
        RunError::TlsCertificateUnavailable => "tls_certificate_unavailable",
        RunError::TlsCertificateInvalid => "tls_certificate_invalid",
        RunError::TlsVerifierUnavailable => "tls_verifier_unavailable",
        RunError::TlsHandshakeFailed => "tls_handshake_failed",
        RunError::InterfaceQueryFailed => "interface_query_failed",
        RunError::DependencyUnavailable => "dependency_unavailable",
        RunError::UnsupportedPlatform => "unsupported_platform",
        RunError::PingFailed => "ping_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExecutionStatus, WebDiagnosticsData};

    #[test]
    fn destination_created_during_the_run_is_not_replaced() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("report.json");
        let destination = JsonReportDestination::prepare(&path).unwrap();
        fs::write(&path, "created by another process").unwrap();

        let error = destination.write(&sample_workflow_result()).unwrap_err();

        assert_eq!(error, ReportExportError::DestinationExists);
        assert_eq!(error.exit_code(), 2);
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "created by another process"
        );
    }

    #[test]
    fn report_omits_observer_target_and_unstructured_step_data() {
        let result = sample_workflow_result();
        let report = SanitizedWorkflowReport::from_result(&result).unwrap();
        let json = serde_json::to_value(report).unwrap();
        let contents = json.to_string();

        assert_eq!(json["target"]["display"], "[redacted]");
        assert_eq!(json["target"]["redacted"], true);
        assert_eq!(json["steps"][0]["status_code"], 200);
        assert!(json.get("observer").is_none());
        assert!(json.get("data").is_none());
        assert!(json["steps"][0].get("data").is_none());
        for sensitive in [
            "report-test-host",
            "internal.example",
            "10.0.0.25",
            "private-path",
            "private-header",
            "private",
        ] {
            assert!(!contents.contains(sensitive), "report leaked {sensitive}");
        }
    }

    #[test]
    fn report_projections_do_not_copy_untrusted_workflow_text_or_identifiers() {
        let mut result = sample_workflow_result();
        result.tool_id = "internal.example";
        if let Some(OutputData::WebDiagnostics(data)) = result.data.as_mut() {
            data.summary = "internal.example/private-path token=private".into();
            data.next_checks = vec!["credential=private-header".into()];
            data.steps[0].id = "internal.example";
            data.steps[0].reason = Some("password=private-path".into());
        }

        let json =
            serde_json::to_string(&SanitizedWorkflowReport::from_result(&result).unwrap()).unwrap();
        let text = SanitizedTextReport::from_result(&result).unwrap();
        let html = render_html_report(&text);
        let csv = render_csv_report(&text);

        for report in [&json, &html, &csv] {
            for secret in [
                "internal.example",
                "private-path",
                "private-header",
                "report-test-host",
                "10.0.0.25",
            ] {
                assert!(!report.contains(secret), "report leaked {secret}: {report}");
            }
            assert!(report.contains("unknown"));
        }
        assert!(json.contains("All applicable protocol layers completed successfully."));
    }

    #[test]
    fn html_report_escapes_workflow_text_and_redacts_sensitive_fields() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("report.html");
        let mut result = sample_workflow_result();
        if let Some(OutputData::WebDiagnostics(data)) = result.data.as_mut() {
            data.summary = "Observed <healthy>, & safe \"report\"".into();
        }

        HtmlReportDestination::prepare(&path)
            .unwrap()
            .write(&result)
            .unwrap();
        let html = fs::read_to_string(path).unwrap();

        assert!(html.contains("All applicable protocol layers completed successfully."));
        assert!(!html.contains("Observed"));
        assert_eq!(
            escape_html("<healthy>, & safe \"report\""),
            "&lt;healthy&gt;, &amp; safe &quot;report&quot;"
        );
        assert!(html.contains("[redacted]"));
        for sensitive in [
            "report-test-host",
            "internal.example",
            "10.0.0.25",
            "private-path",
            "private-header",
        ] {
            assert!(!html.contains(sensitive), "HTML report leaked {sensitive}");
        }
    }

    #[test]
    fn csv_report_quotes_workflow_text_and_redacts_sensitive_fields() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("report.csv");
        let mut result = sample_workflow_result();
        if let Some(OutputData::WebDiagnostics(data)) = result.data.as_mut() {
            data.summary = "Observed <healthy>, & safe \"report\"".into();
        }

        CsvReportDestination::prepare(&path)
            .unwrap()
            .write(&result)
            .unwrap();
        let csv = fs::read_to_string(path).unwrap();

        assert!(csv.contains("\"All applicable protocol layers completed successfully.\""));
        assert!(!csv.contains("Observed"));
        let mut quoted = String::new();
        csv_row(&mut quoted, ["Observed <healthy>, & safe \"report\""]);
        assert_eq!(quoted, "\"Observed <healthy>, & safe \"\"report\"\"\"\r\n");
        assert!(csv.contains("\"[redacted]\""));
        for sensitive in [
            "report-test-host",
            "internal.example",
            "10.0.0.25",
            "private-path",
            "private-header",
        ] {
            assert!(!csv.contains(sensitive), "CSV report leaked {sensitive}");
        }
    }

    #[test]
    fn csv_cells_with_leading_whitespace_or_bom_are_formula_safe() {
        let mut csv = String::new();

        csv_row(
            &mut csv,
            [
                "  =1+1",
                "\t-1+2",
                "\n+1",
                "\u{feff}@SUM(A1:A2)",
                "  ＝1+1",
                "\u{00a0}＋2",
                "  －3",
                "＠SUM(A1:A2)",
                "normal value",
            ],
        );

        assert!(csv.contains("\"'  =1+1\""));
        assert!(csv.contains("\"'\t-1+2\""));
        assert!(csv.contains("\"'\n+1\""));
        assert!(csv.contains("\"'\u{feff}@SUM(A1:A2)\""));
        assert!(csv.contains("\"'  ＝1+1\""));
        assert!(csv.contains("\"'\u{00a0}＋2\""));
        assert!(csv.contains("\"'  －3\""));
        assert!(csv.contains("\"'＠SUM(A1:A2)\""));
        assert!(csv.contains("\"normal value\""));
    }

    #[test]
    fn generic_tool_report_omits_raw_tool_data() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("generic.html");
        let result = sample_generic_result();

        HtmlReportDestination::prepare(&path)
            .unwrap()
            .write(&result)
            .unwrap();
        let html = fs::read_to_string(path).unwrap();

        assert!(html.contains("network.dns"));
        assert!(html.contains("[redacted]"));
        assert!(!html.contains("sensitive.example"));
        assert!(!html.contains("203.0.113.20"));
        assert!(!html.contains("report-test-host"));
    }

    #[test]
    fn html_destination_refuses_a_race_time_overwrite() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("report.html");
        let destination = HtmlReportDestination::prepare(&path).unwrap();
        fs::write(&path, "created by another process").unwrap();

        let error = destination.write(&sample_workflow_result()).unwrap_err();

        assert_eq!(error, ReportExportError::DestinationExists);
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "created by another process"
        );
    }

    #[test]
    fn prepared_report_file_enforces_the_report_size_limit() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("too-large.csv");
        let oversized = vec![b'x'; MAX_REPORT_BYTES + 1];

        let error = PreparedReportFile::prepare(&path)
            .unwrap()
            .write(&oversized)
            .unwrap_err();

        assert_eq!(error, ReportExportError::TooLarge);
        assert!(!path.exists());
    }

    fn sample_generic_result() -> RunResult {
        RunResult {
            schema_version: 1,
            tool_id: "network.dns",
            observer: Some("report-test-host".into()),
            target: "sensitive.example".into(),
            execution_status: ExecutionStatus::Completed,
            finding: Some(Finding::Available),
            data: Some(OutputData::Dns(crate::DnsData {
                hostname: "sensitive.example".into(),
                records: vec![crate::DnsRecord {
                    record_type: "A".into(),
                    answers: vec!["203.0.113.20".into()],
                }],
            })),
            duration_ms: 9,
            error: None,
        }
    }

    fn sample_workflow_result() -> RunResult {
        RunResult {
            schema_version: 1,
            tool_id: "workflow.web_diagnostics",
            observer: Some("report-test-host".into()),
            target: "https://internal.example/private-path?token=private".into(),
            execution_status: ExecutionStatus::Completed,
            finding: Some(Finding::Healthy),
            data: Some(OutputData::WebDiagnostics(WebDiagnosticsData {
                url: "https://internal.example/private-path?token=private".into(),
                hostname: "internal.example".into(),
                port: 443,
                selected_address: Some("10.0.0.25".into()),
                steps: vec![WorkflowStep {
                    id: "http",
                    status: WorkflowStepStatus::Completed,
                    finding: Some(Finding::Healthy),
                    duration_ms: 10,
                    reason: None,
                    data: Some(serde_json::json!({
                        "status_code": 200,
                        "headers": { "X-Internal-Token": "private-header" },
                        "tls_subject": "CN=internal.example",
                    })),
                }],
                summary: "Observed <healthy>, & safe \"report\"".into(),
                next_checks: Vec::new(),
            })),
            duration_ms: 25,
            error: None,
        }
    }
}
