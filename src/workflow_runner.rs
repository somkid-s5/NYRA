//! Sequential executor for validated, typed workflow definitions.
//!
//! Tools are supplied through WorkflowStepExecutor, so CLI and TUI callers
//! can adapt the same shared executor without putting tool behavior here.
//! The runner executes at most one step at a time and never retries.

use crate::workflow_definition::{
    MAX_STEP_TIMEOUT_SECONDS, WorkflowBinding, WorkflowCondition, WorkflowDefinition,
    WorkflowFinding, WorkflowPredicate, WorkflowStepDefinition, WorkflowStepStatus,
    WorkflowToolSchema, WorkflowValidationError, WorkflowValue, value_matches_type,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    pin::Pin,
    time::Duration,
};
use tokio::time::{Instant, timeout_at};

pub const WORKFLOW_RESULT_SCHEMA_VERSION: u8 = 1;

/// Safe status information emitted while a workflow advances.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WorkflowProgress {
    StepStarted {
        step_index: usize,
        total_steps: usize,
        step_id: String,
    },
    StepFinished {
        step_index: usize,
        total_steps: usize,
        step: WorkflowRunStep,
    },
}

/// A sanitized per-step outcome. Parameter and output values are never copied
/// into this displayable result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowRunStep {
    pub id: String,
    pub status: WorkflowStepStatus,
    pub finding: Option<WorkflowFinding>,
    pub duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<WorkflowRunIssue>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowRunIssue {
    ConditionNotMet,
    StepTimedOut,
    WorkflowTimedOut,
    MissingInput { name: String },
    MissingStepOutput { step_id: String, output: String },
    InvalidToolOutput,
    ToolFailed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionStatus {
    Completed,
    Failed,
    TimedOut,
    Cancelled,
}

/// The aggregate status and safe step summaries for one workflow execution.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkflowRunResult {
    pub schema_version: u8,
    pub workflow_id: String,
    pub execution_status: WorkflowExecutionStatus,
    pub steps: Vec<WorkflowRunStep>,
    pub duration_ms: u128,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorkflowRunError {
    InvalidDefinition(Vec<WorkflowValidationError>),
    MissingRequiredInput,
    UnknownInput,
    InvalidInputValue,
}

impl std::fmt::Display for WorkflowRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDefinition(errors) => {
                write!(
                    f,
                    "workflow definition is invalid ({} errors)",
                    errors.len()
                )
            }
            Self::MissingRequiredInput => f.write_str("a required workflow input is missing"),
            Self::UnknownInput => f.write_str("an undeclared workflow input was supplied"),
            Self::InvalidInputValue => f.write_str("a workflow input value is invalid"),
        }
    }
}

impl std::error::Error for WorkflowRunError {}

/// Typed output returned by a registered tool adapter.
///
/// The runner validates output names and values against the trusted schema
/// before making them available to a later step.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowToolOutcome {
    pub status: WorkflowStepStatus,
    pub finding: Option<WorkflowFinding>,
    pub outputs: BTreeMap<String, WorkflowValue>,
}

impl WorkflowToolOutcome {
    pub fn new(
        status: WorkflowStepStatus,
        finding: Option<WorkflowFinding>,
        outputs: BTreeMap<String, WorkflowValue>,
    ) -> Self {
        Self {
            status,
            finding,
            outputs,
        }
    }
}

pub type WorkflowToolFuture<'a> = Pin<Box<dyn Future<Output = WorkflowToolOutcome> + Send + 'a>>;

/// Adapter boundary for NYRA's common tool executor.
///
/// Implementations receive only a registered tool ID and validated typed
/// parameters. The runner enforces the timeout independently and cancels by
/// dropping the returned future, so adapters must not leave detached work
/// running after that future is dropped.
pub trait WorkflowStepExecutor: Sync {
    fn execute<'a>(
        &'a self,
        tool_id: &'a str,
        parameters: BTreeMap<String, WorkflowValue>,
        timeout: Duration,
    ) -> WorkflowToolFuture<'a>;
}

/// Validate and run an ordered workflow with one active tool at a time.
///
/// Inputs are typed values keyed by declared input name. Missing optional
/// inputs use defaults when available and otherwise stay absent. The
/// cancellation future resolves when the caller requests stop. Progress
/// contains only step IDs/status/findings/durations, never parameter values.
pub async fn run_workflow<E, C>(
    definition: &WorkflowDefinition,
    tool_catalog: &[WorkflowToolSchema],
    inputs: BTreeMap<String, WorkflowValue>,
    executor: &E,
    cancellation: C,
    mut progress: Option<&mut dyn FnMut(WorkflowProgress)>,
) -> Result<WorkflowRunResult, WorkflowRunError>
where
    E: WorkflowStepExecutor,
    C: Future<Output = ()> + Send,
{
    definition
        .validate(tool_catalog)
        .map_err(WorkflowRunError::InvalidDefinition)?;
    let resolved_inputs = resolve_inputs(definition, inputs)?;
    let schemas = tool_catalog
        .iter()
        .map(|schema| (schema.id.as_str(), schema))
        .collect::<HashMap<_, _>>();

    let started = Instant::now();
    let workflow_deadline = started + Duration::from_secs(definition.timeout_seconds);
    let mut cancellation = Box::pin(cancellation);
    let mut step_results = Vec::<WorkflowRunStep>::with_capacity(definition.steps.len());
    let mut outputs_by_step = HashMap::<String, BTreeMap<String, WorkflowValue>>::new();
    let mut terminal_status = None;

    for (step_index, step) in definition.steps.iter().enumerate() {
        if Instant::now() >= workflow_deadline {
            terminal_status = Some(WorkflowExecutionStatus::TimedOut);
            append_skipped_steps(
                &definition.steps[step_index..],
                step_index,
                definition.steps.len(),
                WorkflowRunIssue::WorkflowTimedOut,
                &mut step_results,
                &mut progress,
            );
            break;
        }

        // Poll cancellation between steps as well as while an adapter runs.
        let cancelled_before_step = tokio::select! {
            biased;
            _ = cancellation.as_mut() => true,
            _ = tokio::task::yield_now() => false,
        };
        if cancelled_before_step {
            terminal_status = Some(WorkflowExecutionStatus::Cancelled);
            append_skipped_steps(
                &definition.steps[step_index..],
                step_index,
                definition.steps.len(),
                WorkflowRunIssue::Cancelled,
                &mut step_results,
                &mut progress,
            );
            break;
        }

        if step
            .run_if
            .as_ref()
            .is_some_and(|condition| !evaluate_condition(condition, &step_results))
        {
            let record = WorkflowRunStep {
                id: step.id.clone(),
                status: WorkflowStepStatus::Skipped,
                finding: None,
                duration_ms: 0,
                issue: Some(WorkflowRunIssue::ConditionNotMet),
            };
            emit_progress(
                &mut progress,
                WorkflowProgress::StepFinished {
                    step_index,
                    total_steps: definition.steps.len(),
                    step: record.clone(),
                },
            );
            step_results.push(record);
            continue;
        }

        let tool_schema = *schemas
            .get(step.tool.as_str())
            .expect("definition validation checks registered tool IDs");
        let parameters =
            match resolve_parameters(step, tool_schema, &resolved_inputs, &outputs_by_step) {
                Ok(parameters) => parameters,
                Err(issue) => {
                    let record = WorkflowRunStep {
                        id: step.id.clone(),
                        status: WorkflowStepStatus::Failed,
                        finding: None,
                        duration_ms: 0,
                        issue: Some(issue),
                    };
                    emit_progress(
                        &mut progress,
                        WorkflowProgress::StepFinished {
                            step_index,
                            total_steps: definition.steps.len(),
                            step: record.clone(),
                        },
                    );
                    step_results.push(record);
                    continue;
                }
            };

        let now = Instant::now();
        let overall_remaining = workflow_deadline.saturating_duration_since(now);
        if overall_remaining.is_zero() {
            terminal_status = Some(WorkflowExecutionStatus::TimedOut);
            append_skipped_steps(
                &definition.steps[step_index..],
                step_index,
                definition.steps.len(),
                WorkflowRunIssue::WorkflowTimedOut,
                &mut step_results,
                &mut progress,
            );
            break;
        }
        let configured_timeout = Duration::from_secs(
            step.timeout_seconds
                .unwrap_or(definition.timeout_seconds)
                .min(MAX_STEP_TIMEOUT_SECONDS),
        );
        let effective_timeout = configured_timeout.min(overall_remaining);
        let step_deadline = now + effective_timeout;

        emit_progress(
            &mut progress,
            WorkflowProgress::StepStarted {
                step_index,
                total_steps: definition.steps.len(),
                step_id: step.id.clone(),
            },
        );
        let step_started = Instant::now();
        let execution = executor.execute(&step.tool, parameters, effective_timeout);
        enum Completion {
            Cancelled,
            TimedOut,
            Outcome(WorkflowToolOutcome),
        }
        let completion = tokio::select! {
            biased;
            _ = cancellation.as_mut() => Completion::Cancelled,
            result = timeout_at(step_deadline, execution) => match result {
                Ok(outcome) => Completion::Outcome(outcome),
                Err(_) => Completion::TimedOut,
            },
        };
        let duration_ms = step_started.elapsed().as_millis();

        match completion {
            Completion::Cancelled => {
                let record = WorkflowRunStep {
                    id: step.id.clone(),
                    status: WorkflowStepStatus::Cancelled,
                    finding: None,
                    duration_ms,
                    issue: Some(WorkflowRunIssue::Cancelled),
                };
                emit_progress(
                    &mut progress,
                    WorkflowProgress::StepFinished {
                        step_index,
                        total_steps: definition.steps.len(),
                        step: record.clone(),
                    },
                );
                step_results.push(record);
                terminal_status = Some(WorkflowExecutionStatus::Cancelled);
                append_skipped_steps(
                    &definition.steps[step_index + 1..],
                    step_index + 1,
                    definition.steps.len(),
                    WorkflowRunIssue::Cancelled,
                    &mut step_results,
                    &mut progress,
                );
                break;
            }
            Completion::TimedOut => {
                let overall_timed_out = Instant::now() >= workflow_deadline;
                let issue = if overall_timed_out {
                    WorkflowRunIssue::WorkflowTimedOut
                } else {
                    WorkflowRunIssue::StepTimedOut
                };
                let record = WorkflowRunStep {
                    id: step.id.clone(),
                    status: WorkflowStepStatus::Completed,
                    finding: Some(WorkflowFinding::TimedOut),
                    duration_ms,
                    issue: Some(issue),
                };
                emit_progress(
                    &mut progress,
                    WorkflowProgress::StepFinished {
                        step_index,
                        total_steps: definition.steps.len(),
                        step: record.clone(),
                    },
                );
                step_results.push(record);
                if overall_timed_out {
                    terminal_status = Some(WorkflowExecutionStatus::TimedOut);
                    append_skipped_steps(
                        &definition.steps[step_index + 1..],
                        step_index + 1,
                        definition.steps.len(),
                        WorkflowRunIssue::WorkflowTimedOut,
                        &mut step_results,
                        &mut progress,
                    );
                    break;
                }
            }
            Completion::Outcome(mut outcome) => {
                let mut issue = None;
                if !validate_tool_outputs(tool_schema, &outcome.outputs) {
                    outcome.status = WorkflowStepStatus::Failed;
                    outcome.finding = None;
                    outcome.outputs.clear();
                    issue = Some(WorkflowRunIssue::InvalidToolOutput);
                } else if outcome.status == WorkflowStepStatus::Failed {
                    issue = Some(WorkflowRunIssue::ToolFailed);
                }
                if outcome.status != WorkflowStepStatus::Completed {
                    outcome.outputs.clear();
                    outcome.finding = None;
                }
                if outcome.status == WorkflowStepStatus::Cancelled {
                    terminal_status = Some(WorkflowExecutionStatus::Cancelled);
                } else {
                    outputs_by_step.insert(step.id.clone(), outcome.outputs);
                }
                let record = WorkflowRunStep {
                    id: step.id.clone(),
                    status: outcome.status,
                    finding: outcome.finding,
                    duration_ms,
                    issue,
                };
                emit_progress(
                    &mut progress,
                    WorkflowProgress::StepFinished {
                        step_index,
                        total_steps: definition.steps.len(),
                        step: record.clone(),
                    },
                );
                step_results.push(record);
                if terminal_status == Some(WorkflowExecutionStatus::Cancelled) {
                    append_skipped_steps(
                        &definition.steps[step_index + 1..],
                        step_index + 1,
                        definition.steps.len(),
                        WorkflowRunIssue::Cancelled,
                        &mut step_results,
                        &mut progress,
                    );
                    break;
                }
            }
        }
    }

    let execution_status = terminal_status.unwrap_or_else(|| {
        if step_results
            .iter()
            .any(|step| step.status == WorkflowStepStatus::Failed)
        {
            WorkflowExecutionStatus::Failed
        } else {
            WorkflowExecutionStatus::Completed
        }
    });
    Ok(WorkflowRunResult {
        schema_version: WORKFLOW_RESULT_SCHEMA_VERSION,
        workflow_id: definition.id.clone(),
        execution_status,
        steps: step_results,
        duration_ms: started.elapsed().as_millis(),
    })
}

/// Validate supplied inputs and fill declared defaults for a workflow.
pub fn resolve_inputs(
    definition: &WorkflowDefinition,
    mut supplied: BTreeMap<String, WorkflowValue>,
) -> Result<BTreeMap<String, WorkflowValue>, WorkflowRunError> {
    let declared = definition
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect::<HashSet<_>>();
    if supplied
        .keys()
        .any(|name| !declared.contains(name.as_str()))
    {
        return Err(WorkflowRunError::UnknownInput);
    }

    let mut resolved = BTreeMap::new();
    for input in &definition.inputs {
        let value = supplied
            .remove(&input.name)
            .or_else(|| input.default_value.clone());
        match value {
            Some(value) if value_matches_type(&value, input.value_type) => {
                resolved.insert(input.name.clone(), value);
            }
            Some(_) => return Err(WorkflowRunError::InvalidInputValue),
            None if input.required => {
                return Err(WorkflowRunError::MissingRequiredInput);
            }
            None => {}
        }
    }
    Ok(resolved)
}

fn resolve_parameters(
    step: &WorkflowStepDefinition,
    schema: &WorkflowToolSchema,
    inputs: &BTreeMap<String, WorkflowValue>,
    outputs_by_step: &HashMap<String, BTreeMap<String, WorkflowValue>>,
) -> Result<BTreeMap<String, WorkflowValue>, WorkflowRunIssue> {
    let mut parameters = BTreeMap::new();
    for binding in &step.parameters {
        let parameter = schema
            .parameters
            .get(&binding.parameter)
            .expect("definition validation checks parameter names");
        let value = match &binding.source {
            WorkflowBinding::Input { name } => inputs.get(name).cloned(),
            WorkflowBinding::Literal { value } => Some(value.clone()),
            WorkflowBinding::StepOutput { step_id, output } => outputs_by_step
                .get(step_id)
                .and_then(|outputs| outputs.get(output))
                .cloned(),
        };
        if let Some(value) = value {
            parameters.insert(binding.parameter.clone(), value);
        } else {
            match &binding.source {
                WorkflowBinding::StepOutput { step_id, output } => {
                    return Err(WorkflowRunIssue::MissingStepOutput {
                        step_id: step_id.clone(),
                        output: output.clone(),
                    });
                }
                WorkflowBinding::Input { name } if parameter.required => {
                    return Err(WorkflowRunIssue::MissingInput { name: name.clone() });
                }
                WorkflowBinding::Input { .. } | WorkflowBinding::Literal { .. } => {}
            }
        }
    }
    Ok(parameters)
}

fn evaluate_condition(condition: &WorkflowCondition, prior_steps: &[WorkflowRunStep]) -> bool {
    let predicate_matches = |predicate: &WorkflowPredicate| match predicate {
        WorkflowPredicate::StepStatus { step_id, equals } => prior_steps
            .iter()
            .find(|step| &step.id == step_id)
            .is_some_and(|step| step.status == *equals),
        WorkflowPredicate::StepFinding { step_id, equals } => prior_steps
            .iter()
            .find(|step| &step.id == step_id)
            .is_some_and(|step| step.finding == Some(*equals)),
    };
    match condition {
        WorkflowCondition::All { predicates } => predicates.iter().all(predicate_matches),
        WorkflowCondition::Any { predicates } => predicates.iter().any(predicate_matches),
    }
}

fn validate_tool_outputs(
    schema: &WorkflowToolSchema,
    outputs: &BTreeMap<String, WorkflowValue>,
) -> bool {
    outputs.iter().all(|(name, value)| {
        schema
            .outputs
            .get(name)
            .is_some_and(|value_type| value_matches_type(value, *value_type))
    })
}

fn append_skipped_steps(
    steps: &[WorkflowStepDefinition],
    first_index: usize,
    total_steps: usize,
    issue: WorkflowRunIssue,
    results: &mut Vec<WorkflowRunStep>,
    progress: &mut Option<&mut dyn FnMut(WorkflowProgress)>,
) {
    for (offset, step) in steps.iter().enumerate() {
        let step_index = first_index + offset;
        let record = WorkflowRunStep {
            id: step.id.clone(),
            status: WorkflowStepStatus::Skipped,
            finding: None,
            duration_ms: 0,
            issue: Some(issue.clone()),
        };
        emit_progress(
            progress,
            WorkflowProgress::StepFinished {
                step_index,
                total_steps,
                step: record.clone(),
            },
        );
        results.push(record);
    }
}

fn emit_progress(progress: &mut Option<&mut dyn FnMut(WorkflowProgress)>, event: WorkflowProgress) {
    if let Some(callback) = progress.as_mut() {
        callback(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_definition::WorkflowValueType;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockExecutor {
        calls: Mutex<Vec<(String, BTreeMap<String, WorkflowValue>)>>,
        outcomes: HashMap<String, WorkflowToolOutcome>,
        delays: HashMap<String, Duration>,
    }

    impl WorkflowStepExecutor for MockExecutor {
        fn execute<'a>(
            &'a self,
            tool_id: &'a str,
            parameters: BTreeMap<String, WorkflowValue>,
            _timeout: Duration,
        ) -> WorkflowToolFuture<'a> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .unwrap()
                    .push((tool_id.to_owned(), parameters));
                if let Some(delay) = self.delays.get(tool_id) {
                    tokio::time::sleep(*delay).await;
                }
                self.outcomes.get(tool_id).cloned().unwrap_or_else(|| {
                    WorkflowToolOutcome::new(
                        WorkflowStepStatus::Completed,
                        Some(WorkflowFinding::Available),
                        BTreeMap::new(),
                    )
                })
            })
        }
    }

    #[tokio::test]
    async fn runs_steps_in_order_and_resolves_typed_outputs() {
        let definition = sample_definition(false);
        let executor = MockExecutor {
            outcomes: HashMap::from([(
                "test.producer".into(),
                WorkflowToolOutcome::new(
                    WorkflowStepStatus::Completed,
                    Some(WorkflowFinding::Available),
                    BTreeMap::from([("host".into(), WorkflowValue::Text("example.com".into()))]),
                ),
            )]),
            ..MockExecutor::default()
        };

        let result = run_workflow(
            &definition,
            &schemas(),
            sample_inputs(),
            &executor,
            std::future::pending::<()>(),
            None,
        )
        .await
        .unwrap();
        let calls = executor.calls.lock().unwrap();

        assert_eq!(result.execution_status, WorkflowExecutionStatus::Completed);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "test.producer");
        assert_eq!(calls[1].0, "test.consumer");
        assert_eq!(
            calls[1].1.get("hostname"),
            Some(&WorkflowValue::Text("example.com".into()))
        );
    }

    #[tokio::test]
    async fn condition_skips_step_without_invoking_its_tool() {
        let definition = sample_definition(true);
        let executor = MockExecutor {
            outcomes: HashMap::from([(
                "test.producer".into(),
                WorkflowToolOutcome::new(
                    WorkflowStepStatus::Completed,
                    Some(WorkflowFinding::Refused),
                    BTreeMap::new(),
                ),
            )]),
            ..MockExecutor::default()
        };

        let result = run_workflow(
            &definition,
            &schemas(),
            sample_inputs(),
            &executor,
            std::future::pending::<()>(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(executor.calls.lock().unwrap().len(), 1);
        assert_eq!(result.steps[1].status, WorkflowStepStatus::Skipped);
        assert_eq!(
            result.steps[1].issue,
            Some(WorkflowRunIssue::ConditionNotMet)
        );
    }

    #[tokio::test]
    async fn missing_output_fails_step_without_calling_dependent_tool() {
        let definition = sample_definition(false);
        let executor = MockExecutor::default();

        let result = run_workflow(
            &definition,
            &schemas(),
            sample_inputs(),
            &executor,
            std::future::pending::<()>(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(executor.calls.lock().unwrap().len(), 1);
        assert_eq!(result.steps[1].status, WorkflowStepStatus::Failed);
        assert_eq!(
            result.steps[1].issue,
            Some(WorkflowRunIssue::MissingStepOutput {
                step_id: "resolve".into(),
                output: "host".into(),
            })
        );
        assert_eq!(result.execution_status, WorkflowExecutionStatus::Failed);
    }

    #[tokio::test]
    async fn failed_tool_outcome_does_not_report_a_diagnostic_finding() {
        let mut definition = sample_definition(false);
        definition.steps.truncate(1);
        let executor = MockExecutor {
            outcomes: HashMap::from([(
                "test.producer".into(),
                WorkflowToolOutcome::new(
                    WorkflowStepStatus::Failed,
                    Some(WorkflowFinding::Unhealthy),
                    BTreeMap::new(),
                ),
            )]),
            ..MockExecutor::default()
        };

        let result = run_workflow(
            &definition,
            &schemas(),
            sample_inputs(),
            &executor,
            std::future::pending::<()>(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.execution_status, WorkflowExecutionStatus::Failed);
        assert_eq!(result.steps[0].finding, None);
        assert_eq!(result.steps[0].issue, Some(WorkflowRunIssue::ToolFailed));
    }

    #[tokio::test]
    async fn step_timeout_is_a_finding_and_progress_omits_input_values() {
        let mut definition = sample_definition(false);
        definition.steps.truncate(1);
        definition.steps[0].timeout_seconds = Some(1);
        let executor = MockExecutor {
            delays: HashMap::from([("test.producer".into(), Duration::from_secs(2))]),
            ..MockExecutor::default()
        };
        let serialized = {
            let mut events = Vec::new();
            let mut progress = |event| events.push(event);
            let result = run_workflow(
                &definition,
                &schemas(),
                sample_inputs(),
                &executor,
                std::future::pending::<()>(),
                Some(&mut progress),
            )
            .await
            .unwrap();

            assert_eq!(result.steps[0].finding, Some(WorkflowFinding::TimedOut));
            assert_eq!(result.steps[0].issue, Some(WorkflowRunIssue::StepTimedOut));
            serde_json::to_string(&events).unwrap()
        };
        assert!(!serialized.contains("private.example"));
        assert!(serialized.contains("resolve"));
    }

    #[tokio::test]
    async fn overall_timeout_stops_work_and_marks_remaining_steps_skipped() {
        let mut definition = sample_definition(false);
        definition.timeout_seconds = 1;
        definition.steps[0].timeout_seconds = Some(2);
        let executor = MockExecutor {
            delays: HashMap::from([("test.producer".into(), Duration::from_secs(2))]),
            ..MockExecutor::default()
        };

        let result = run_workflow(
            &definition,
            &schemas(),
            sample_inputs(),
            &executor,
            std::future::pending::<()>(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(result.execution_status, WorkflowExecutionStatus::TimedOut);
        assert_eq!(result.steps[0].finding, Some(WorkflowFinding::TimedOut));
        assert_eq!(
            result.steps[0].issue,
            Some(WorkflowRunIssue::WorkflowTimedOut)
        );
        assert_eq!(result.steps[1].status, WorkflowStepStatus::Skipped);
        assert_eq!(
            result.steps[1].issue,
            Some(WorkflowRunIssue::WorkflowTimedOut)
        );
        assert_eq!(executor.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn cancellation_preserves_prior_step_and_marks_remaining_steps() {
        let definition = sample_definition(false);
        let executor = MockExecutor {
            outcomes: HashMap::from([(
                "test.producer".into(),
                WorkflowToolOutcome::new(
                    WorkflowStepStatus::Completed,
                    Some(WorkflowFinding::Available),
                    BTreeMap::from([("host".into(), WorkflowValue::Text("example.com".into()))]),
                ),
            )]),
            ..MockExecutor::default()
        };

        let cancellation = async {
            loop {
                if executor.calls.lock().unwrap().len() == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        };
        let result = run_workflow(
            &definition,
            &schemas(),
            sample_inputs(),
            &executor,
            cancellation,
            None,
        )
        .await
        .unwrap();

        assert_eq!(executor.calls.lock().unwrap().len(), 1);
        assert_eq!(result.execution_status, WorkflowExecutionStatus::Cancelled);
        assert_eq!(result.steps[0].status, WorkflowStepStatus::Completed);
        assert_eq!(result.steps[1].status, WorkflowStepStatus::Skipped);
    }

    fn sample_definition(condition_on_open: bool) -> WorkflowDefinition {
        use crate::workflow_definition::{
            CURRENT_WORKFLOW_VERSION, WorkflowParameterBinding, WorkflowPredicate,
            WorkflowStepDefinition,
        };

        WorkflowDefinition {
            id: "test-flow".into(),
            version: CURRENT_WORKFLOW_VERSION,
            description: "Resolve then consume a hostname".into(),
            timeout_seconds: 5,
            inputs: vec![crate::workflow_definition::WorkflowInputDefinition {
                name: "target_host".into(),
                value_type: WorkflowValueType::Hostname,
                required: true,
                default_value: None,
            }],
            steps: vec![
                WorkflowStepDefinition {
                    id: "resolve".into(),
                    tool: "test.producer".into(),
                    parameters: vec![WorkflowParameterBinding {
                        parameter: "hostname".into(),
                        source: WorkflowBinding::Input {
                            name: "target_host".into(),
                        },
                    }],
                    run_if: None,
                    timeout_seconds: Some(2),
                },
                WorkflowStepDefinition {
                    id: "consume".into(),
                    tool: "test.consumer".into(),
                    parameters: vec![WorkflowParameterBinding {
                        parameter: "hostname".into(),
                        source: WorkflowBinding::StepOutput {
                            step_id: "resolve".into(),
                            output: "host".into(),
                        },
                    }],
                    run_if: condition_on_open.then(|| WorkflowCondition::All {
                        predicates: vec![WorkflowPredicate::StepFinding {
                            step_id: "resolve".into(),
                            equals: WorkflowFinding::Open,
                        }],
                    }),
                    timeout_seconds: None,
                },
            ],
        }
    }

    fn schemas() -> Vec<WorkflowToolSchema> {
        use crate::workflow_definition::WorkflowToolParameterSchema;

        vec![
            WorkflowToolSchema::new(
                "test.producer",
                true,
                BTreeMap::from([(
                    "hostname".into(),
                    WorkflowToolParameterSchema::required(WorkflowValueType::Hostname),
                )]),
                BTreeMap::from([("host".into(), WorkflowValueType::Hostname)]),
            ),
            WorkflowToolSchema::new(
                "test.consumer",
                true,
                BTreeMap::from([(
                    "hostname".into(),
                    WorkflowToolParameterSchema::required(WorkflowValueType::Hostname),
                )]),
                BTreeMap::new(),
            ),
        ]
    }

    fn sample_inputs() -> BTreeMap<String, WorkflowValue> {
        BTreeMap::from([(
            "target_host".into(),
            WorkflowValue::Text("private.example".into()),
        )])
    }
}
