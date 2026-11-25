//! Serializable, typed workflow definitions and conservative validation.
//!
//! This module describes a sequential workflow. It does not execute tools or
//! evaluate user-provided expressions. Conditions are a bounded AST over prior
//! step outcomes, and parameter bindings can only reference declared inputs,
//! literals, or typed outputs of earlier steps.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::IpAddr,
};

pub const CURRENT_WORKFLOW_VERSION: u16 = 1;
pub const MAX_WORKFLOW_ID_BYTES: usize = 64;
pub const MAX_IDENTIFIER_BYTES: usize = 64;
pub const MAX_DESCRIPTION_BYTES: usize = 512;
pub const MAX_INPUTS: usize = 32;
pub const MAX_STEPS: usize = 64;
pub const MAX_BINDINGS_PER_STEP: usize = 32;
pub const MAX_CONDITION_PREDICATES: usize = 16;
pub const MAX_WORKFLOW_TIMEOUT_SECONDS: u64 = 300;
pub const MAX_STEP_TIMEOUT_SECONDS: u64 = 60;
pub const MAX_TEXT_VALUE_BYTES: usize = 1024;

fn default_workflow_timeout_seconds() -> u64 {
    30
}

fn default_required() -> bool {
    true
}

/// A versioned definition that can be validated against NYRA's registered
/// read-only tools before a future executor accepts it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDefinition {
    pub id: String,
    pub version: u16,
    pub description: String,
    #[serde(default = "default_workflow_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub inputs: Vec<WorkflowInputDefinition>,
    pub steps: Vec<WorkflowStepDefinition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowInputDefinition {
    pub name: String,
    pub value_type: WorkflowValueType,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default, rename = "default")]
    pub default_value: Option<WorkflowValue>,
}

/// The finite set of value types allowed in workflow inputs, tool parameters,
/// literals, and output schemas. Types are invariant: a string is never
/// silently coerced to a URL, port, or timeout.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowValueType {
    String,
    /// A DNS hostname; IP literals are rejected.
    DnsHostname,
    /// A DNS hostname or an IPv4/IPv6 literal.
    Hostname,
    IpAddress,
    Url,
    FilePath,
    RecordTypes,
    Port,
    TimeoutSeconds,
    Count,
    Integer,
    Boolean,
}

/// A literal value in a workflow definition. String-like types share the text
/// representation, while semantic validation is performed against the
/// declared `WorkflowValueType`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged)]
pub enum WorkflowValue {
    Text(String),
    Integer(i64),
    Boolean(bool),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowStepDefinition {
    pub id: String,
    pub tool: String,
    #[serde(default)]
    pub parameters: Vec<WorkflowParameterBinding>,
    #[serde(default)]
    pub run_if: Option<WorkflowCondition>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowParameterBinding {
    pub parameter: String,
    pub source: WorkflowBinding,
}

/// The only sources from which a tool parameter may be populated.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowBinding {
    Input { name: String },
    Literal { value: WorkflowValue },
    StepOutput { step_id: String, output: String },
}

/// Conditions are intentionally shallow: a single `all`/`any` group of
/// predicates over previous step outcomes. Free-form expressions and nested
/// recursion are not part of the definition format.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operator", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowCondition {
    All { predicates: Vec<WorkflowPredicate> },
    Any { predicates: Vec<WorkflowPredicate> },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowPredicate {
    StepStatus {
        step_id: String,
        equals: WorkflowStepStatus,
    },
    StepFinding {
        step_id: String,
        equals: WorkflowFinding,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepStatus {
    Completed,
    Skipped,
    Failed,
    Cancelled,
}

/// Findings currently produced by NYRA's shared result model.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFinding {
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

/// Runtime metadata for a registered tool. This schema is supplied by NYRA's
/// trusted registry, never loaded from a workflow file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowToolSchema {
    pub id: String,
    pub read_only: bool,
    pub parameters: BTreeMap<String, WorkflowToolParameterSchema>,
    pub outputs: BTreeMap<String, WorkflowValueType>,
}

impl WorkflowToolSchema {
    pub fn new(
        id: impl Into<String>,
        read_only: bool,
        parameters: BTreeMap<String, WorkflowToolParameterSchema>,
        outputs: BTreeMap<String, WorkflowValueType>,
    ) -> Self {
        Self {
            id: id.into(),
            read_only,
            parameters,
            outputs,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkflowToolParameterSchema {
    pub value_type: WorkflowValueType,
    pub required: bool,
}

impl WorkflowToolParameterSchema {
    pub const fn required(value_type: WorkflowValueType) -> Self {
        Self {
            value_type,
            required: true,
        }
    }

    pub const fn optional(value_type: WorkflowValueType) -> Self {
        Self {
            value_type,
            required: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowValidationErrorKind {
    InvalidIdentifier,
    UnsupportedVersion,
    InvalidDescription,
    LimitExceeded,
    EmptySteps,
    InvalidTimeout,
    DuplicateInput,
    DuplicateToolSchema,
    InvalidDefault,
    DuplicateStep,
    DuplicateBinding,
    UnknownTool,
    ToolNotReadOnly,
    UnknownInput,
    OptionalInputMayBeMissing,
    UnknownParameter,
    MissingRequiredParameter,
    TypeMismatch,
    UnknownStepReference,
    FutureStepReference,
    UnknownToolOutput,
    EmptyCondition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowValidationError {
    pub path: String,
    pub kind: WorkflowValidationErrorKind,
}

impl std::fmt::Display for WorkflowValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.kind)
    }
}

impl std::fmt::Display for WorkflowValidationErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidIdentifier => {
                "identifier must use lowercase ASCII letters, digits, `_`, `-`, or valid tool-id dots"
            }
            Self::UnsupportedVersion => "workflow version is not supported",
            Self::InvalidDescription => "description must not be empty or exceed the size limit",
            Self::LimitExceeded => "workflow exceeds a configured size or value limit",
            Self::EmptySteps => "workflow must contain at least one step",
            Self::InvalidTimeout => "timeout is outside the supported range",
            Self::DuplicateInput => "input name is duplicated",
            Self::DuplicateToolSchema => "tool registry contains a duplicate tool ID",
            Self::InvalidDefault => "default value does not match its declared type or constraints",
            Self::DuplicateStep => "step id is duplicated",
            Self::DuplicateBinding => "parameter binding is duplicated",
            Self::UnknownTool => "tool is not registered",
            Self::ToolNotReadOnly => "workflow steps may reference read-only tools only",
            Self::UnknownInput => "input is not declared by this workflow",
            Self::OptionalInputMayBeMissing => {
                "optional input without a default cannot satisfy a required tool parameter"
            }
            Self::UnknownParameter => "parameter is not declared by the registered tool",
            Self::MissingRequiredParameter => "required tool parameter has no binding",
            Self::TypeMismatch => "binding type does not match the declared parameter type",
            Self::UnknownStepReference => "referenced step is not declared",
            Self::FutureStepReference => "conditions and outputs may reference earlier steps only",
            Self::UnknownToolOutput => "output is not declared by the referenced tool",
            Self::EmptyCondition => "condition must contain at least one predicate",
        };
        f.write_str(message)
    }
}

impl std::error::Error for WorkflowValidationError {}

impl WorkflowDefinition {
    /// Validate the user definition against trusted tool schemas.
    ///
    /// The returned errors identify definition paths but do not echo literal
    /// values. Only read-only registered tools are accepted; all references in
    /// conditions and output bindings must point to an earlier ordered step.
    pub fn validate(
        &self,
        tool_catalog: &[WorkflowToolSchema],
    ) -> Result<(), Vec<WorkflowValidationError>> {
        let mut errors = Vec::new();

        if !valid_identifier(&self.id, false) || self.id.len() > MAX_WORKFLOW_ID_BYTES {
            push_error(
                &mut errors,
                "id",
                WorkflowValidationErrorKind::InvalidIdentifier,
            );
        }
        if self.version != CURRENT_WORKFLOW_VERSION {
            push_error(
                &mut errors,
                "version",
                WorkflowValidationErrorKind::UnsupportedVersion,
            );
        }
        if self.description.trim().is_empty() || self.description.len() > MAX_DESCRIPTION_BYTES {
            push_error(
                &mut errors,
                "description",
                WorkflowValidationErrorKind::InvalidDescription,
            );
        }
        if !(1..=MAX_WORKFLOW_TIMEOUT_SECONDS).contains(&self.timeout_seconds) {
            push_error(
                &mut errors,
                "timeout_seconds",
                WorkflowValidationErrorKind::InvalidTimeout,
            );
        }
        if self.inputs.len() > MAX_INPUTS {
            push_error(
                &mut errors,
                "inputs",
                WorkflowValidationErrorKind::LimitExceeded,
            );
            return Err(errors);
        }
        if self.steps.is_empty() {
            push_error(
                &mut errors,
                "steps",
                WorkflowValidationErrorKind::EmptySteps,
            );
        }
        if self.steps.len() > MAX_STEPS {
            push_error(
                &mut errors,
                "steps",
                WorkflowValidationErrorKind::LimitExceeded,
            );
            return Err(errors);
        }

        let mut inputs = HashMap::with_capacity(self.inputs.len());
        for (index, input) in self.inputs.iter().enumerate() {
            let path = format!("inputs[{index}]");
            if !valid_identifier(&input.name, false) {
                push_error(
                    &mut errors,
                    format!("{path}.name"),
                    WorkflowValidationErrorKind::InvalidIdentifier,
                );
            }
            if inputs
                .insert(
                    input.name.as_str(),
                    WorkflowInputSchema {
                        value_type: input.value_type,
                        required: input.required,
                        has_default: input.default_value.is_some(),
                    },
                )
                .is_some()
            {
                push_error(
                    &mut errors,
                    format!("{path}.name"),
                    WorkflowValidationErrorKind::DuplicateInput,
                );
            }
            if let Some(value) = &input.default_value
                && !value_matches_type(value, input.value_type)
            {
                push_error(
                    &mut errors,
                    format!("{path}.default"),
                    WorkflowValidationErrorKind::InvalidDefault,
                );
            }
        }

        let mut tools = HashMap::with_capacity(tool_catalog.len());
        for (tool_index, tool) in tool_catalog.iter().enumerate() {
            if !valid_identifier(&tool.id, true) {
                // Invalid registry metadata cannot be safely addressed by a
                // workflow. Treat it as unavailable rather than trusting it.
                continue;
            }
            if tools.insert(tool.id.as_str(), tool).is_some() {
                push_error(
                    &mut errors,
                    format!("tool_catalog[{tool_index}].id"),
                    WorkflowValidationErrorKind::DuplicateToolSchema,
                );
            }
        }

        let mut step_positions = HashMap::with_capacity(self.steps.len());
        for (index, step) in self.steps.iter().enumerate() {
            if !valid_identifier(&step.id, false) {
                push_error(
                    &mut errors,
                    format!("steps[{index}].id"),
                    WorkflowValidationErrorKind::InvalidIdentifier,
                );
            }
            if step_positions.insert(step.id.as_str(), index).is_some() {
                push_error(
                    &mut errors,
                    format!("steps[{index}].id"),
                    WorkflowValidationErrorKind::DuplicateStep,
                );
            }
        }

        let mut step_tools = Vec::with_capacity(self.steps.len());
        for (index, step) in self.steps.iter().enumerate() {
            let path = format!("steps[{index}]");
            if !valid_identifier(&step.tool, true) {
                push_error(
                    &mut errors,
                    format!("{path}.tool"),
                    WorkflowValidationErrorKind::InvalidIdentifier,
                );
            }
            if let Some(timeout) = step.timeout_seconds
                && !(1..=MAX_STEP_TIMEOUT_SECONDS).contains(&timeout)
            {
                push_error(
                    &mut errors,
                    format!("{path}.timeout_seconds"),
                    WorkflowValidationErrorKind::InvalidTimeout,
                );
            }
            if step.parameters.len() > MAX_BINDINGS_PER_STEP {
                push_error(
                    &mut errors,
                    format!("{path}.parameters"),
                    WorkflowValidationErrorKind::LimitExceeded,
                );
                step_tools.push(None);
                continue;
            }

            let Some(tool_schema) = tools.get(step.tool.as_str()).copied() else {
                push_error(
                    &mut errors,
                    format!("{path}.tool"),
                    WorkflowValidationErrorKind::UnknownTool,
                );
                step_tools.push(None);
                continue;
            };
            if !tool_schema.read_only {
                push_error(
                    &mut errors,
                    format!("{path}.tool"),
                    WorkflowValidationErrorKind::ToolNotReadOnly,
                );
            }
            step_tools.push(Some(tool_schema));
            let binding_context = BindingValidationContext {
                current_step: index,
                inputs: &inputs,
                step_positions: &step_positions,
                step_tools: &step_tools,
            };

            let mut provided = HashSet::with_capacity(step.parameters.len());
            for (binding_index, binding) in step.parameters.iter().enumerate() {
                let binding_path = format!("{path}.parameters[{binding_index}]");
                if !valid_identifier(&binding.parameter, false) {
                    push_error(
                        &mut errors,
                        format!("{binding_path}.parameter"),
                        WorkflowValidationErrorKind::InvalidIdentifier,
                    );
                }
                if !provided.insert(binding.parameter.as_str()) {
                    push_error(
                        &mut errors,
                        format!("{binding_path}.parameter"),
                        WorkflowValidationErrorKind::DuplicateBinding,
                    );
                    continue;
                }

                let Some(parameter_schema) = tool_schema.parameters.get(&binding.parameter) else {
                    push_error(
                        &mut errors,
                        format!("{binding_path}.parameter"),
                        WorkflowValidationErrorKind::UnknownParameter,
                    );
                    continue;
                };
                if let Some(source_type) = binding_source_type(
                    &binding.source,
                    parameter_schema.value_type,
                    parameter_schema.required,
                    &binding_context,
                    &mut errors,
                    &binding_path,
                ) && source_type != parameter_schema.value_type
                {
                    push_error(
                        &mut errors,
                        &binding_path,
                        WorkflowValidationErrorKind::TypeMismatch,
                    );
                }
            }
            for (name, parameter) in &tool_schema.parameters {
                if parameter.required && !provided.contains(name.as_str()) {
                    push_error(
                        &mut errors,
                        format!("{path}.parameters.{name}"),
                        WorkflowValidationErrorKind::MissingRequiredParameter,
                    );
                }
            }
        }

        for (index, step) in self.steps.iter().enumerate() {
            let Some(condition) = &step.run_if else {
                continue;
            };
            let predicates = match condition {
                WorkflowCondition::All { predicates } | WorkflowCondition::Any { predicates } => {
                    predicates
                }
            };
            let path = format!("steps[{index}].run_if");
            if predicates.is_empty() {
                push_error(
                    &mut errors,
                    &path,
                    WorkflowValidationErrorKind::EmptyCondition,
                );
            }
            if predicates.len() > MAX_CONDITION_PREDICATES {
                push_error(
                    &mut errors,
                    &path,
                    WorkflowValidationErrorKind::LimitExceeded,
                );
                continue;
            }
            for (predicate_index, predicate) in predicates.iter().enumerate() {
                validate_step_reference(
                    predicate.step_id(),
                    index,
                    &step_positions,
                    &mut errors,
                    format!("{path}.predicates[{predicate_index}].step_id"),
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl WorkflowPredicate {
    fn step_id(&self) -> &str {
        match self {
            Self::StepStatus { step_id, .. } | Self::StepFinding { step_id, .. } => step_id,
        }
    }
}

struct BindingValidationContext<'a> {
    current_step: usize,
    inputs: &'a HashMap<&'a str, WorkflowInputSchema>,
    step_positions: &'a HashMap<&'a str, usize>,
    step_tools: &'a [Option<&'a WorkflowToolSchema>],
}

#[derive(Clone, Copy)]
struct WorkflowInputSchema {
    value_type: WorkflowValueType,
    required: bool,
    has_default: bool,
}

fn binding_source_type(
    binding: &WorkflowBinding,
    expected_type: WorkflowValueType,
    parameter_required: bool,
    context: &BindingValidationContext<'_>,
    errors: &mut Vec<WorkflowValidationError>,
    path: &str,
) -> Option<WorkflowValueType> {
    match binding {
        WorkflowBinding::Input { name } => match context.inputs.get(name.as_str()) {
            Some(input) => {
                if parameter_required && !input.required && !input.has_default {
                    push_error(
                        errors,
                        format!("{path}.source.name"),
                        WorkflowValidationErrorKind::OptionalInputMayBeMissing,
                    );
                }
                Some(input.value_type)
            }
            None => {
                push_error(
                    errors,
                    format!("{path}.source.name"),
                    WorkflowValidationErrorKind::UnknownInput,
                );
                None
            }
        },
        WorkflowBinding::Literal { value } => {
            if !value_matches_type(value, expected_type) {
                push_error(
                    errors,
                    format!("{path}.source.value"),
                    WorkflowValidationErrorKind::InvalidDefault,
                );
                return None;
            }
            Some(expected_type)
        }
        WorkflowBinding::StepOutput { step_id, output } => {
            if !valid_identifier(output, false) {
                push_error(
                    errors,
                    format!("{path}.source.output"),
                    WorkflowValidationErrorKind::InvalidIdentifier,
                );
            }
            let Some(position) = context.step_positions.get(step_id.as_str()).copied() else {
                push_error(
                    errors,
                    format!("{path}.source.step_id"),
                    WorkflowValidationErrorKind::UnknownStepReference,
                );
                return None;
            };
            if position >= context.current_step {
                push_error(
                    errors,
                    format!("{path}.source.step_id"),
                    WorkflowValidationErrorKind::FutureStepReference,
                );
                return None;
            }
            let source_tool = context.step_tools.get(position).copied().flatten();
            let Some(source_tool) = source_tool else {
                // The referenced prior step's tool schema was already
                // unavailable; its UnknownTool error is sufficient.
                return None;
            };
            match source_tool.outputs.get(output) {
                Some(value_type) => Some(*value_type),
                None => {
                    push_error(
                        errors,
                        format!("{path}.source.output"),
                        WorkflowValidationErrorKind::UnknownToolOutput,
                    );
                    None
                }
            }
        }
    }
}

fn validate_step_reference(
    step_id: &str,
    current_step: usize,
    step_positions: &HashMap<&str, usize>,
    errors: &mut Vec<WorkflowValidationError>,
    path: String,
) {
    match step_positions.get(step_id).copied() {
        Some(position) if position < current_step => {}
        Some(_) => push_error(
            errors,
            path,
            WorkflowValidationErrorKind::FutureStepReference,
        ),
        None => push_error(
            errors,
            path,
            WorkflowValidationErrorKind::UnknownStepReference,
        ),
    }
}

pub(crate) fn value_matches_type(value: &WorkflowValue, value_type: WorkflowValueType) -> bool {
    match (value, value_type) {
        (WorkflowValue::Text(text), WorkflowValueType::String) => {
            text.len() <= MAX_TEXT_VALUE_BYTES && !text.contains('\0')
        }
        (WorkflowValue::Text(text), WorkflowValueType::DnsHostname) => {
            text.parse::<IpAddr>().is_err() && valid_hostname(text)
        }
        (WorkflowValue::Text(text), WorkflowValueType::Hostname) => valid_hostname(text),
        (WorkflowValue::Text(text), WorkflowValueType::IpAddress) => text.parse::<IpAddr>().is_ok(),
        (WorkflowValue::Text(text), WorkflowValueType::Url) => valid_http_url(text),
        (WorkflowValue::Text(text), WorkflowValueType::FilePath) => {
            !text.trim().is_empty() && text.len() <= MAX_TEXT_VALUE_BYTES && !text.contains('\0')
        }
        (WorkflowValue::Text(text), WorkflowValueType::RecordTypes) => valid_record_types(text),
        (WorkflowValue::Integer(value), WorkflowValueType::Port) => (1..=65_535).contains(value),
        (WorkflowValue::Integer(value), WorkflowValueType::TimeoutSeconds) => {
            (1..=MAX_STEP_TIMEOUT_SECONDS as i64).contains(value)
        }
        (WorkflowValue::Integer(value), WorkflowValueType::Count) => (1..=10).contains(value),
        (WorkflowValue::Integer(_), WorkflowValueType::Integer) => true,
        (WorkflowValue::Boolean(_), WorkflowValueType::Boolean) => true,
        _ => false,
    }
}

fn valid_record_types(value: &str) -> bool {
    let types = value
        .split(',')
        .map(|kind| kind.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();
    !types.is_empty()
        && types.len() <= 5
        && types
            .iter()
            .all(|kind| matches!(kind.as_str(), "A" | "AAAA" | "CNAME" | "MX" | "TXT"))
}

fn valid_hostname(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 253
        || value.contains('\0')
        || value.chars().any(char::is_whitespace)
        || value.contains('/')
        || value.contains('\\')
        || value.contains('@')
        || value.contains('?')
        || value.contains('#')
    {
        return false;
    }
    let hostname = value.strip_suffix('.').unwrap_or(value);
    if hostname.ends_with('.') {
        return false;
    }
    value.parse::<std::net::IpAddr>().is_ok()
        || hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn valid_http_url(value: &str) -> bool {
    if value.len() > MAX_TEXT_VALUE_BYTES || value.contains('\0') {
        return false;
    }
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
}

fn valid_identifier(value: &str, allow_dots: bool) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    value.split('.').all(|segment| {
        if segment.is_empty() || (!allow_dots && value.contains('.')) {
            return false;
        }
        let mut bytes = segment.bytes();
        let Some(first) = bytes.next() else {
            return false;
        };
        first.is_ascii_lowercase()
            && bytes.all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            })
    })
}

fn push_error(
    errors: &mut Vec<WorkflowValidationError>,
    path: impl Into<String>,
    kind: WorkflowValidationErrorKind,
) {
    errors.push(WorkflowValidationError {
        path: path.into(),
        kind,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typed_ordered_read_only_workflow() {
        let definition = valid_definition();
        let catalog = catalog();

        assert_eq!(definition.validate(&catalog), Ok(()));
    }

    #[test]
    fn rejects_duplicate_registered_tool_ids() {
        let definition = valid_definition();
        let mut catalog = catalog();
        catalog.push(WorkflowToolSchema::new(
            "network.dns",
            false,
            BTreeMap::new(),
            BTreeMap::new(),
        ));

        let errors = definition.validate(&catalog).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|error| error.kind == WorkflowValidationErrorKind::DuplicateToolSchema)
        );
    }

    #[test]
    fn rejects_duplicate_ids_and_conditions_that_reference_current_or_future_steps() {
        let mut definition = valid_definition();
        definition.steps[1].id = "dns".into();
        definition.steps[1].run_if = Some(WorkflowCondition::All {
            predicates: vec![WorkflowPredicate::StepStatus {
                step_id: "tcp".into(),
                equals: WorkflowStepStatus::Completed,
            }],
        });

        let errors = definition.validate(&catalog()).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::DuplicateStep })
        );
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::FutureStepReference })
        );
    }

    #[test]
    fn rejects_parameter_type_mismatch_and_invalid_port_literal() {
        let mut definition = valid_definition();
        definition.inputs[0].value_type = WorkflowValueType::Url;
        definition.steps[1].parameters = vec![
            WorkflowParameterBinding {
                parameter: "hostname".into(),
                source: WorkflowBinding::Input {
                    name: "target_hostname".into(),
                },
            },
            WorkflowParameterBinding {
                parameter: "port".into(),
                source: WorkflowBinding::Literal {
                    value: WorkflowValue::Integer(65_536),
                },
            },
        ];

        let errors = definition.validate(&catalog()).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::TypeMismatch })
        );
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::InvalidDefault })
        );
    }

    #[test]
    fn optional_input_needs_a_default_when_bound_to_a_required_parameter() {
        let mut definition = valid_definition();
        definition.inputs[0].required = false;

        let errors = definition.validate(&catalog()).unwrap_err();
        assert!(
            errors.iter().any(|error| {
                error.kind == WorkflowValidationErrorKind::OptionalInputMayBeMissing
            })
        );

        definition.inputs[0].default_value = Some(WorkflowValue::Text("example.com".into()));
        assert!(definition.validate(&catalog()).is_ok());
    }

    #[test]
    fn hostname_validation_allows_one_terminal_root_dot_only() {
        assert!(valid_hostname("host.example."));
        assert!(!valid_hostname("host.example.."));
    }

    #[test]
    fn validates_semantic_types_used_by_tool_registry_inputs() {
        assert!(value_matches_type(
            &WorkflowValue::Text("example.com".into()),
            WorkflowValueType::DnsHostname
        ));
        assert!(!value_matches_type(
            &WorkflowValue::Text("127.0.0.1".into()),
            WorkflowValueType::DnsHostname
        ));
        assert!(value_matches_type(
            &WorkflowValue::Text("127.0.0.1".into()),
            WorkflowValueType::IpAddress
        ));
        assert!(!value_matches_type(
            &WorkflowValue::Text("invalid-address".into()),
            WorkflowValueType::IpAddress
        ));
        assert!(value_matches_type(
            &WorkflowValue::Text("A, AAAA".into()),
            WorkflowValueType::RecordTypes
        ));
        assert!(!value_matches_type(
            &WorkflowValue::Text("A,BOGUS".into()),
            WorkflowValueType::RecordTypes
        ));
        assert!(!value_matches_type(
            &WorkflowValue::Text("  ".into()),
            WorkflowValueType::FilePath
        ));
        assert!(!value_matches_type(
            &WorkflowValue::Integer(11),
            WorkflowValueType::Count
        ));
    }

    #[test]
    fn rejects_unknown_tools_non_read_only_tools_and_unknown_outputs() {
        let mut definition = valid_definition();
        definition.steps[0].tool = "system.mutate".into();
        definition.steps[1].parameters[0].source = WorkflowBinding::StepOutput {
            step_id: "dns".into(),
            output: "secret_field".into(),
        };
        let mut catalog = catalog();
        catalog.push(WorkflowToolSchema::new(
            "system.mutate",
            false,
            BTreeMap::new(),
            BTreeMap::new(),
        ));

        let errors = definition.validate(&catalog).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::ToolNotReadOnly })
        );
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::UnknownToolOutput })
        );
    }

    #[test]
    fn serde_round_trip_preserves_structured_conditions_and_rejects_expression_strings() {
        let definition = valid_definition();
        let encoded = serde_json::to_string(&definition).unwrap();
        let decoded: WorkflowDefinition = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, definition);

        let free_form = r#"{
            "id":"web-check",
            "version":1,
            "description":"Check a web target",
            "timeout_seconds":30,
            "inputs":[],
            "steps":[{
                "id":"dns",
                "tool":"network.dns",
                "parameters":[],
                "run_if":"target.is_ip or dns.has_addresses",
                "timeout_seconds":null
            }]
        }"#;
        assert!(serde_json::from_str::<WorkflowDefinition>(free_form).is_err());
    }

    #[test]
    fn validates_defaults_and_condition_size() {
        let mut definition = valid_definition();
        definition.inputs[0].default_value = Some(WorkflowValue::Text(
            "https://user:password@example.com/path".into(),
        ));
        definition.steps[1].run_if = Some(WorkflowCondition::Any {
            predicates: (0..=MAX_CONDITION_PREDICATES)
                .map(|_| WorkflowPredicate::StepStatus {
                    step_id: "dns".into(),
                    equals: WorkflowStepStatus::Completed,
                })
                .collect(),
        });

        let errors = definition.validate(&catalog()).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::InvalidDefault })
        );
        assert!(
            errors
                .iter()
                .any(|error| { error.kind == WorkflowValidationErrorKind::LimitExceeded })
        );
    }

    fn valid_definition() -> WorkflowDefinition {
        WorkflowDefinition {
            id: "web-check".into(),
            version: CURRENT_WORKFLOW_VERSION,
            description: "Resolve a target, then check its TCP service".into(),
            timeout_seconds: 30,
            inputs: vec![
                WorkflowInputDefinition {
                    name: "target_hostname".into(),
                    value_type: WorkflowValueType::DnsHostname,
                    required: true,
                    default_value: None,
                },
                WorkflowInputDefinition {
                    name: "target_port".into(),
                    value_type: WorkflowValueType::Port,
                    required: true,
                    default_value: None,
                },
            ],
            steps: vec![
                WorkflowStepDefinition {
                    id: "dns".into(),
                    tool: "network.dns".into(),
                    parameters: vec![WorkflowParameterBinding {
                        parameter: "hostname".into(),
                        source: WorkflowBinding::Input {
                            name: "target_hostname".into(),
                        },
                    }],
                    run_if: None,
                    timeout_seconds: Some(5),
                },
                WorkflowStepDefinition {
                    id: "tcp".into(),
                    tool: "network.tcp".into(),
                    parameters: vec![
                        WorkflowParameterBinding {
                            parameter: "hostname".into(),
                            source: WorkflowBinding::StepOutput {
                                step_id: "dns".into(),
                                output: "hostname".into(),
                            },
                        },
                        WorkflowParameterBinding {
                            parameter: "port".into(),
                            source: WorkflowBinding::Input {
                                name: "target_port".into(),
                            },
                        },
                    ],
                    run_if: Some(WorkflowCondition::All {
                        predicates: vec![WorkflowPredicate::StepStatus {
                            step_id: "dns".into(),
                            equals: WorkflowStepStatus::Completed,
                        }],
                    }),
                    timeout_seconds: None,
                },
            ],
        }
    }

    fn catalog() -> Vec<WorkflowToolSchema> {
        vec![
            WorkflowToolSchema::new(
                "network.dns",
                true,
                BTreeMap::from([(
                    "hostname".into(),
                    WorkflowToolParameterSchema::required(WorkflowValueType::DnsHostname),
                )]),
                BTreeMap::from([("hostname".into(), WorkflowValueType::Hostname)]),
            ),
            WorkflowToolSchema::new(
                "network.tcp",
                true,
                BTreeMap::from([
                    (
                        "hostname".into(),
                        WorkflowToolParameterSchema::required(WorkflowValueType::Hostname),
                    ),
                    (
                        "port".into(),
                        WorkflowToolParameterSchema::required(WorkflowValueType::Port),
                    ),
                ]),
                BTreeMap::new(),
            ),
        ]
    }
}
