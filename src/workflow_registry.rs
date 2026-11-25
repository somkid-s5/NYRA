//! Typed workflow schemas projected from NYRA's trusted tool registry.
//!
//! This is intentionally a narrow projection: only scalar inputs and
//! explicitly selected scalar outputs are bindable in workflows. The raw
//! result data, collections, headers, interface addresses, and file paths are
//! not exposed as workflow outputs here.

use crate::{
    CATALOG, InputKind, OutputSchema, RiskLevel,
    workflow_definition::{WorkflowToolParameterSchema, WorkflowToolSchema, WorkflowValueType},
};
use std::collections::BTreeMap;

/// Build the trusted schemas used to validate user-defined workflows.
///
/// Tool metadata defaults make a parameter optional in the workflow binding
/// layer. The adapter that executes the tool must apply the same registered
/// default when the workflow does not bind that parameter.
pub fn workflow_tool_catalog() -> Vec<WorkflowToolSchema> {
    CATALOG
        .iter()
        .map(|tool| {
            let parameters = tool
                .input_fields
                .iter()
                .map(|field| {
                    let value_type = workflow_input_type(field.kind);
                    let parameter = if field.required && field.default.is_none() {
                        WorkflowToolParameterSchema::required(value_type)
                    } else {
                        WorkflowToolParameterSchema::optional(value_type)
                    };
                    (field.name.to_owned(), parameter)
                })
                .collect();

            WorkflowToolSchema::new(
                tool.id,
                matches!(tool.risk, RiskLevel::R0 | RiskLevel::R1),
                parameters,
                workflow_output_types(tool.output_schema),
            )
        })
        .collect()
}

fn workflow_input_type(kind: InputKind) -> WorkflowValueType {
    match kind {
        InputKind::Host => WorkflowValueType::Hostname,
        InputKind::DnsHostname => WorkflowValueType::DnsHostname,
        InputKind::IpAddress => WorkflowValueType::IpAddress,
        InputKind::Port => WorkflowValueType::Port,
        InputKind::Url => WorkflowValueType::Url,
        InputKind::Path => WorkflowValueType::FilePath,
        InputKind::RecordTypes => WorkflowValueType::RecordTypes,
        InputKind::TimeoutSeconds => WorkflowValueType::TimeoutSeconds,
        InputKind::Count => WorkflowValueType::Count,
    }
}

fn workflow_output_types(schema: OutputSchema) -> BTreeMap<String, WorkflowValueType> {
    use WorkflowValueType::{Boolean, Hostname, Integer, Port, String, Url};

    let fields: &[(&str, WorkflowValueType)] = match schema {
        OutputSchema::FindingOnly | OutputSchema::InterfaceData => &[],
        OutputSchema::DnsData => &[("hostname", Hostname)],
        OutputSchema::HttpData => &[
            ("url", Url),
            ("status_code", Integer),
            ("elapsed_ms", Integer),
        ],
        OutputSchema::TlsData => &[
            ("host", Hostname),
            ("port", Port),
            ("subject", String),
            ("issuer", String),
            ("not_before_unix", Integer),
            ("not_after_unix", Integer),
            ("trust_validation", String),
        ],
        OutputSchema::PingData => &[
            ("host", Hostname),
            ("count", Integer),
            ("reachable", Boolean),
        ],
        OutputSchema::SystemInfoData => &[
            ("os", String),
            ("architecture", String),
            ("hostname", String),
            ("uptime_seconds", Integer),
        ],
        OutputSchema::CpuData => &[
            ("logical_cores", Integer),
            ("physical_cores", Integer),
            ("frequency_mhz", Integer),
        ],
        OutputSchema::MemoryData => &[
            ("total_bytes", Integer),
            ("used_bytes", Integer),
            ("available_bytes", Integer),
        ],
        // Disk results contain paths and volume names. A count is sufficient
        // for branch/binding use without exposing those identifying values.
        OutputSchema::DiskData => &[("volume_count", Integer)],
        // The digest and size can be reused safely. Do not expose the path.
        OutputSchema::Sha256Data => &[("sha256", String), ("size_bytes", Integer)],
        OutputSchema::WebDiagnosticsData => &[("hostname", Hostname), ("port", Port)],
    };

    fields
        .iter()
        .map(|(name, value_type)| ((*name).to_owned(), *value_type))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_definition::WorkflowValueType;
    use std::collections::HashSet;

    #[test]
    fn creates_one_unique_workflow_schema_for_each_registered_tool() {
        let schemas = workflow_tool_catalog();
        let ids = schemas
            .iter()
            .map(|schema| schema.id.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(schemas.len(), CATALOG.len());
        assert_eq!(ids.len(), CATALOG.len());
        assert!(CATALOG.iter().all(|tool| ids.contains(tool.id)));
    }

    #[test]
    fn maps_typed_inputs_and_allows_registered_defaults_to_be_omitted() {
        let schemas = workflow_tool_catalog();
        let tcp = schemas
            .iter()
            .find(|schema| schema.id == "network.tcp")
            .unwrap();
        assert_eq!(
            tcp.parameters["host"].value_type,
            WorkflowValueType::Hostname
        );
        assert!(tcp.parameters["host"].required);
        assert_eq!(tcp.parameters["port"].value_type, WorkflowValueType::Port);
        assert!(tcp.parameters["port"].required);
        assert_eq!(
            tcp.parameters["timeout"].value_type,
            WorkflowValueType::TimeoutSeconds
        );
        assert!(!tcp.parameters["timeout"].required);

        let reverse_dns = schemas
            .iter()
            .find(|schema| schema.id == "network.reverse_dns")
            .unwrap();
        assert_eq!(
            reverse_dns.parameters["address"].value_type,
            WorkflowValueType::IpAddress
        );
        assert!(reverse_dns.parameters["address"].required);

        let dns = schemas
            .iter()
            .find(|schema| schema.id == "network.dns")
            .unwrap();
        assert_eq!(
            dns.parameters["hostname"].value_type,
            WorkflowValueType::DnsHostname
        );
        assert_eq!(
            dns.parameters["record_types"].value_type,
            WorkflowValueType::RecordTypes
        );

        let file_hash = schemas
            .iter()
            .find(|schema| schema.id == "file.sha256")
            .unwrap();
        assert_eq!(
            file_hash.parameters["path"].value_type,
            WorkflowValueType::FilePath
        );
        assert_eq!(
            schemas
                .iter()
                .find(|schema| schema.id == "network.ping")
                .unwrap()
                .parameters["count"]
                .value_type,
            WorkflowValueType::Count
        );
    }

    #[test]
    fn marks_only_registered_diagnostic_risk_levels_as_read_only() {
        let schemas = workflow_tool_catalog();

        for (tool, schema) in CATALOG.iter().zip(schemas) {
            assert_eq!(
                schema.read_only,
                matches!(tool.risk, RiskLevel::R0 | RiskLevel::R1),
                "unexpected read-only projection for {}",
                tool.id
            );
        }
    }

    #[test]
    fn exposes_only_explicit_scalar_workflow_outputs() {
        let schemas = workflow_tool_catalog();
        let by_id = |id: &str| schemas.iter().find(|schema| schema.id == id).unwrap();

        assert_eq!(
            by_id("web.http").outputs["status_code"],
            WorkflowValueType::Integer
        );
        assert!(!by_id("web.http").outputs.contains_key("headers"));
        assert!(!by_id("network.dns").outputs.contains_key("records"));
        assert!(
            !by_id("network.interfaces")
                .outputs
                .contains_key("interfaces")
        );
        assert!(!by_id("system.disk").outputs.contains_key("volumes"));
        assert!(!by_id("file.sha256").outputs.contains_key("path"));
        assert!(by_id("file.sha256").outputs.contains_key("sha256"));
    }
}
