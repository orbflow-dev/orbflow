// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Compliance export formatters for audit records.
//!
//! Converts audit trail data into industry-standard compliance formats
//! (SOC 2, HIPAA, PCI DSS) as downloadable CSV files.

use serde::{Deserialize, Serialize};

use crate::audit::{AuditRecord, verify_record};
use crate::error::OrbflowError;

/// Supported compliance export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceFormat {
    Soc2,
    Hipaa,
    Pci,
}

impl ComplianceFormat {
    /// Parses a format string into a `ComplianceFormat`.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "soc2" => Some(Self::Soc2),
            "hipaa" => Some(Self::Hipaa),
            "pci" => Some(Self::Pci),
            _ => None,
        }
    }
}

/// Trait for formatting audit records into compliance exports.
pub trait ComplianceExporter: Send + Sync {
    /// The compliance format this exporter produces.
    fn format(&self) -> ComplianceFormat;
    /// Exports audit records as bytes (typically CSV).
    fn export(&self, records: &[AuditRecord]) -> Result<Vec<u8>, OrbflowError>;
    /// The MIME content type for the export (e.g., `text/csv`).
    fn content_type(&self) -> &str;
    /// The file extension for the export (e.g., `csv`).
    fn file_extension(&self) -> &str;
}

/// Returns the appropriate exporter for the given format.
pub fn exporter_for(format: ComplianceFormat) -> Box<dyn ComplianceExporter> {
    match format {
        ComplianceFormat::Soc2 => Box::new(Soc2Exporter),
        ComplianceFormat::Hipaa => Box::new(HipaaExporter),
        ComplianceFormat::Pci => Box::new(PciExporter),
    }
}

// ─── CSV Helpers ─────────────────────────────────────────────────────────────

/// Escapes a CSV field value: wraps in double quotes if it contains commas,
/// quotes, or newlines, and doubles any internal quotes.
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

/// Writes a CSV row from a slice of field values.
fn csv_row(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| csv_escape(f))
        .collect::<Vec<_>>()
        .join(",")
}

/// Extracts the event type from serialized event JSON data.
fn extract_event_type(event_data: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(event_data)
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extracts the instance ID from serialized event JSON data.
fn extract_instance_id(event_data: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(event_data)
        .ok()
        .and_then(|v| {
            // Try common nested paths for instance_id.
            v.get("instance_id")
                .or_else(|| v.get("base").and_then(|b| b.get("instance_id")))
                .and_then(|id| id.as_str())
                .map(String::from)
        })
        .unwrap_or_default()
}

/// Extracts the timestamp from serialized event JSON data.
fn extract_timestamp(event_data: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(event_data)
        .ok()
        .and_then(|v| {
            v.get("timestamp")
                .or_else(|| v.get("base").and_then(|b| b.get("timestamp")))
                .and_then(|t| t.as_str())
                .map(String::from)
        })
        .unwrap_or_default()
}

/// Extracts the node_id from serialized event JSON data (if present).
fn extract_node_id(event_data: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(event_data)
        .ok()
        .and_then(|v| v.get("node_id").and_then(|n| n.as_str()).map(String::from))
        .unwrap_or_default()
}

/// Determines the signature status for a record.
/// Since AuditRecord does not carry a signature field, we verify the hash.
fn signature_status(record: &AuditRecord) -> &'static str {
    if verify_record(record) {
        "verified"
    } else {
        "invalid"
    }
}

/// Maps event types to data access categories for HIPAA.
fn data_access_type(event_type: &str) -> &'static str {
    match event_type {
        "node.completed" | "node.started" => "process",
        "instance.started" => "create",
        "instance.completed" | "instance.failed" | "instance.cancelled" => "lifecycle",
        "node.approved" | "node.rejected" | "node.approval_requested" => "authorization",
        _ => "system",
    }
}

/// Maps event types to credential access categories for PCI.
fn credential_access(event_type: &str) -> &'static str {
    match event_type {
        "node.completed" | "node.started" => "potential",
        "node.approved" | "node.rejected" => "authorization",
        _ => "none",
    }
}

// ─── SOC 2 Exporter ─────────────────────────────────────────────────────────

/// SOC 2 compliance exporter — produces CSV with security control columns.
pub struct Soc2Exporter;

impl ComplianceExporter for Soc2Exporter {
    fn format(&self) -> ComplianceFormat {
        ComplianceFormat::Soc2
    }

    fn export(&self, records: &[AuditRecord]) -> Result<Vec<u8>, OrbflowError> {
        let mut output = String::new();

        // Header row.
        output.push_str(&csv_row(&[
            "timestamp",
            "event_type",
            "instance_id",
            "actor",
            "action",
            "resource",
            "event_hash",
            "prev_hash",
            "signature_status",
        ]));
        output.push('\n');

        for record in records {
            let event_type = extract_event_type(&record.event_data);
            let instance_id = extract_instance_id(&record.event_data);
            let timestamp = extract_timestamp(&record.event_data);
            let node_id = extract_node_id(&record.event_data);
            let sig_status = signature_status(record);

            // Map event type to action/actor/resource.
            let action = &event_type;
            let actor = "system";
            let resource = if node_id.is_empty() {
                &instance_id
            } else {
                &node_id
            };

            output.push_str(&csv_row(&[
                &timestamp,
                &event_type,
                &instance_id,
                actor,
                action,
                resource,
                &record.event_hash,
                &record.prev_hash,
                sig_status,
            ]));
            output.push('\n');
        }

        Ok(output.into_bytes())
    }

    fn content_type(&self) -> &str {
        "text/csv"
    }

    fn file_extension(&self) -> &str {
        "csv"
    }
}

// ─── HIPAA Exporter ──────────────────────────────────────────────────────────

/// HIPAA compliance exporter — produces CSV with PHI-relevant columns.
pub struct HipaaExporter;

impl ComplianceExporter for HipaaExporter {
    fn format(&self) -> ComplianceFormat {
        ComplianceFormat::Hipaa
    }

    fn export(&self, records: &[AuditRecord]) -> Result<Vec<u8>, OrbflowError> {
        let mut output = String::new();

        output.push_str(&csv_row(&[
            "timestamp",
            "event_type",
            "instance_id",
            "data_access_type",
            "user_id",
            "action",
            "hash_verified",
        ]));
        output.push('\n');

        for record in records {
            let event_type = extract_event_type(&record.event_data);
            let instance_id = extract_instance_id(&record.event_data);
            let timestamp = extract_timestamp(&record.event_data);
            let access_type = data_access_type(&event_type);
            let hash_ok = verify_record(record).to_string();

            output.push_str(&csv_row(&[
                &timestamp,
                &event_type,
                &instance_id,
                access_type,
                "system",
                &event_type,
                &hash_ok,
            ]));
            output.push('\n');
        }

        Ok(output.into_bytes())
    }

    fn content_type(&self) -> &str {
        "text/csv"
    }

    fn file_extension(&self) -> &str {
        "csv"
    }
}

// ─── PCI Exporter ────────────────────────────────────────────────────────────

/// PCI DSS compliance exporter — produces CSV with cardholder-data columns.
pub struct PciExporter;

impl ComplianceExporter for PciExporter {
    fn format(&self) -> ComplianceFormat {
        ComplianceFormat::Pci
    }

    fn export(&self, records: &[AuditRecord]) -> Result<Vec<u8>, OrbflowError> {
        let mut output = String::new();

        output.push_str(&csv_row(&[
            "timestamp",
            "event_type",
            "instance_id",
            "credential_access",
            "user_id",
            "action",
            "hash_verified",
        ]));
        output.push('\n');

        for record in records {
            let event_type = extract_event_type(&record.event_data);
            let instance_id = extract_instance_id(&record.event_data);
            let timestamp = extract_timestamp(&record.event_data);
            let cred_access = credential_access(&event_type);
            let hash_ok = verify_record(record).to_string();

            output.push_str(&csv_row(&[
                &timestamp,
                &event_type,
                &instance_id,
                cred_access,
                "system",
                &event_type,
                &hash_ok,
            ]));
            output.push('\n');
        }

        Ok(output.into_bytes())
    }

    fn content_type(&self) -> &str {
        "text/csv"
    }

    fn file_extension(&self) -> &str {
        "csv"
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{GENESIS_HASH, create_audit_record};

    fn make_test_event(event_type: &str, instance_id: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "type": event_type,
            "base": {
                "instance_id": instance_id,
                "timestamp": "2026-03-22T10:00:00Z",
            },
            "node_id": "node_1",
        }))
        .unwrap()
    }

    fn make_test_records(n: usize) -> Vec<AuditRecord> {
        let mut records = Vec::new();
        let mut prev_hash = GENESIS_HASH.to_string();

        let event_types = [
            "instance.started",
            "node.queued",
            "node.started",
            "node.completed",
            "instance.completed",
        ];

        for i in 0..n {
            let event_type = event_types[i % event_types.len()];
            let data = make_test_event(event_type, "inst-001");
            let record = create_audit_record(data, &prev_hash, i as u64);
            prev_hash = record.event_hash.clone();
            records.push(record);
        }

        records
    }

    #[test]
    fn soc2_exporter_produces_valid_csv_with_correct_headers() {
        let records = make_test_records(3);
        let exporter = Soc2Exporter;

        assert_eq!(exporter.format(), ComplianceFormat::Soc2);
        assert_eq!(exporter.content_type(), "text/csv");
        assert_eq!(exporter.file_extension(), "csv");

        let output = exporter.export(&records).unwrap();
        let csv = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(lines.len(), 4); // header + 3 data rows
        assert_eq!(
            lines[0],
            "timestamp,event_type,instance_id,actor,action,resource,event_hash,prev_hash,signature_status"
        );

        // Verify each data row has 9 fields.
        for line in &lines[1..] {
            // Simple field count (not splitting inside quotes, but our test data has no commas).
            let fields: Vec<&str> = line.split(',').collect();
            assert_eq!(fields.len(), 9, "Expected 9 fields, got: {line}");
        }
    }

    #[test]
    fn hipaa_exporter_produces_valid_csv_with_correct_headers() {
        let records = make_test_records(2);
        let exporter = HipaaExporter;

        assert_eq!(exporter.format(), ComplianceFormat::Hipaa);
        assert_eq!(exporter.content_type(), "text/csv");

        let output = exporter.export(&records).unwrap();
        let csv = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(lines.len(), 3); // header + 2 data rows
        assert_eq!(
            lines[0],
            "timestamp,event_type,instance_id,data_access_type,user_id,action,hash_verified"
        );
    }

    #[test]
    fn pci_exporter_produces_valid_csv_with_correct_headers() {
        let records = make_test_records(2);
        let exporter = PciExporter;

        assert_eq!(exporter.format(), ComplianceFormat::Pci);
        assert_eq!(exporter.content_type(), "text/csv");

        let output = exporter.export(&records).unwrap();
        let csv = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[0],
            "timestamp,event_type,instance_id,credential_access,user_id,action,hash_verified"
        );
    }

    #[test]
    fn export_with_empty_records_returns_header_only() {
        let records: Vec<AuditRecord> = Vec::new();

        for format in [
            ComplianceFormat::Soc2,
            ComplianceFormat::Hipaa,
            ComplianceFormat::Pci,
        ] {
            let exporter = exporter_for(format);
            let output = exporter.export(&records).unwrap();
            let csv = String::from_utf8(output).unwrap();
            let lines: Vec<&str> = csv.lines().collect();
            assert_eq!(lines.len(), 1, "Expected header only for {:?}", format);
        }
    }

    #[test]
    fn csv_escape_handles_special_characters() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("has,comma"), "\"has,comma\"");
        assert_eq!(csv_escape("has\"quote"), "\"has\"\"quote\"");
        assert_eq!(csv_escape("has\nnewline"), "\"has\nnewline\"");
        assert_eq!(csv_escape(""), "");
    }

    #[test]
    fn compliance_format_from_str_opt_parses_correctly() {
        assert_eq!(
            ComplianceFormat::from_str_opt("soc2"),
            Some(ComplianceFormat::Soc2)
        );
        assert_eq!(
            ComplianceFormat::from_str_opt("SOC2"),
            Some(ComplianceFormat::Soc2)
        );
        assert_eq!(
            ComplianceFormat::from_str_opt("hipaa"),
            Some(ComplianceFormat::Hipaa)
        );
        assert_eq!(
            ComplianceFormat::from_str_opt("pci"),
            Some(ComplianceFormat::Pci)
        );
        assert_eq!(ComplianceFormat::from_str_opt("unknown"), None);
    }

    #[test]
    fn exporter_for_returns_correct_exporter() {
        assert_eq!(
            exporter_for(ComplianceFormat::Soc2).format(),
            ComplianceFormat::Soc2
        );
        assert_eq!(
            exporter_for(ComplianceFormat::Hipaa).format(),
            ComplianceFormat::Hipaa
        );
        assert_eq!(
            exporter_for(ComplianceFormat::Pci).format(),
            ComplianceFormat::Pci
        );
    }

    #[test]
    fn soc2_exporter_includes_hash_verification() {
        let records = make_test_records(1);
        let exporter = Soc2Exporter;
        let output = exporter.export(&records).unwrap();
        let csv = String::from_utf8(output).unwrap();

        // The signature_status should be "verified" for valid records.
        assert!(
            csv.contains("verified"),
            "Expected 'verified' in CSV output"
        );
    }

    #[test]
    fn hipaa_data_access_type_mapping() {
        assert_eq!(data_access_type("instance.started"), "create");
        assert_eq!(data_access_type("node.completed"), "process");
        assert_eq!(data_access_type("node.approved"), "authorization");
        assert_eq!(data_access_type("instance.completed"), "lifecycle");
        assert_eq!(data_access_type("something.else"), "system");
    }

    #[test]
    fn pci_credential_access_mapping() {
        assert_eq!(credential_access("node.started"), "potential");
        assert_eq!(credential_access("node.approved"), "authorization");
        assert_eq!(credential_access("instance.started"), "none");
    }
}
