// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Wave 2 integration tests for audit trail integrity, Merkle tree proofs,
//! compliance CSV exports, and Ed25519 digital signatures.
//!
//! These tests exercise cross-module interactions within orbflow-core that unit
//! tests in individual modules do not cover.

use orbflow_core::audit::{
    AuditChainError, AuditRecord, AuditSigner, Ed25519Signer, GENESIS_HASH, MerkleTree,
    compute_event_hash, create_audit_record, verify_chain, verify_record,
};
use orbflow_core::compliance::{
    ComplianceExporter, ComplianceFormat, HipaaExporter, PciExporter, Soc2Exporter, exporter_for,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_event_json(event_type: &str, instance_id: &str, seq: usize) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "type": event_type,
        "base": {
            "instance_id": instance_id,
            "timestamp": format!("2026-03-22T10:{:02}:00Z", seq),
        },
        "node_id": format!("node_{}", seq),
    }))
    .unwrap()
}

fn make_chain(n: usize) -> Vec<AuditRecord> {
    let mut records = Vec::new();
    let mut prev_hash = GENESIS_HASH.to_string();
    let event_types = [
        "instance.started",
        "node.started",
        "node.completed",
        "instance.completed",
    ];

    for i in 0..n {
        let event_type = event_types[i % event_types.len()];
        let data = make_event_json(event_type, "inst-001", i);
        let record = create_audit_record(data, &prev_hash, i as u64);
        prev_hash = record.event_hash.clone();
        records.push(record);
    }

    records
}

// ===========================================================================
// Hash Chain Tampering Detection
// ===========================================================================

#[test]
fn test_hash_chain_detects_data_tampering() {
    let mut chain = make_chain(5);
    assert!(verify_chain(&chain).is_ok(), "valid chain should verify");

    // Tamper with middle record's data.
    chain[2].event_data = b"tampered-data".to_vec();

    let err = verify_chain(&chain).unwrap_err();
    assert_eq!(
        err,
        AuditChainError::TamperedRecord { index: 2 },
        "should detect tampered record at index 2"
    );
}

#[test]
fn test_hash_chain_detects_reordering() {
    let mut chain = make_chain(4);
    assert!(verify_chain(&chain).is_ok());

    // Swap records 1 and 2 — chain linkage should break.
    chain.swap(1, 2);

    let err = verify_chain(&chain).unwrap_err();
    // After swap, record at index 1 has wrong hash or broken chain.
    assert!(
        matches!(
            err,
            AuditChainError::TamperedRecord { .. }
                | AuditChainError::BrokenChain { .. }
                | AuditChainError::InvalidSequence { .. }
        ),
        "should detect reordered records, got: {err:?}"
    );
}

#[test]
fn test_hash_chain_detects_deletion() {
    let mut chain = make_chain(5);
    assert!(verify_chain(&chain).is_ok());

    // Remove the middle record — chain linkage breaks at the gap.
    chain.remove(2);

    let err = verify_chain(&chain).unwrap_err();
    assert!(
        matches!(
            err,
            AuditChainError::BrokenChain { .. } | AuditChainError::InvalidSequence { .. }
        ),
        "should detect deleted record, got: {err:?}"
    );
}

#[test]
fn test_hash_chain_detects_insertion() {
    let mut chain = make_chain(4);
    assert!(verify_chain(&chain).is_ok());

    // Insert a forged record in the middle.
    let forged = create_audit_record(b"forged".to_vec(), &chain[1].event_hash, 2);
    chain.insert(2, forged);

    let err = verify_chain(&chain).unwrap_err();
    // The original record at index 3 now has wrong sequence or broken chain.
    assert!(
        err != AuditChainError::InvalidGenesis,
        "should detect insertion, got: {err:?}"
    );
}

#[test]
fn test_hash_chain_single_record_valid() {
    let chain = make_chain(1);
    assert!(verify_chain(&chain).is_ok());
}

// ===========================================================================
// Merkle Tree End-to-End Proofs
// ===========================================================================

#[test]
fn test_merkle_proof_end_to_end_8_leaves() {
    let hashes: Vec<String> = (0..8)
        .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
        .collect();

    let tree = MerkleTree::build(&hashes);
    let root = tree.root().to_string();

    // Verify proof for every leaf.
    for (i, hash) in hashes.iter().enumerate() {
        let proof = tree.proof(i);
        assert!(
            !proof.is_empty(),
            "proof for leaf {i} should have sibling nodes"
        );
        assert!(
            MerkleTree::verify_proof(hash, &proof, &root),
            "proof for leaf {i} should verify against root"
        );
    }
}

#[test]
fn test_merkle_proof_tampered_leaf_fails() {
    let hashes: Vec<String> = (0..8)
        .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
        .collect();

    let tree = MerkleTree::build(&hashes);
    let root = tree.root().to_string();

    // Get valid proof for leaf 3.
    let proof = tree.proof(3);

    // Tamper with the leaf hash.
    let tampered_leaf = compute_event_hash(b"tampered", GENESIS_HASH);
    assert!(
        !MerkleTree::verify_proof(&tampered_leaf, &proof, &root),
        "tampered leaf should not verify"
    );

    // Original should still work.
    assert!(
        MerkleTree::verify_proof(&hashes[3], &proof, &root),
        "original leaf should still verify"
    );
}

#[test]
fn test_merkle_proof_wrong_root_fails() {
    let hashes: Vec<String> = (0..4)
        .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
        .collect();

    let tree = MerkleTree::build(&hashes);
    let proof = tree.proof(0);

    let wrong_root = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    assert!(
        !MerkleTree::verify_proof(&hashes[0], &proof, wrong_root),
        "proof against wrong root should fail"
    );
}

#[test]
fn test_merkle_tree_from_audit_chain() {
    // Build a Merkle tree from actual audit record hashes — cross-module integration.
    let chain = make_chain(8);
    let leaf_hashes: Vec<String> = chain.iter().map(|r| r.event_hash.clone()).collect();

    let tree = MerkleTree::build(&leaf_hashes);
    let root = tree.root().to_string();
    assert!(!root.is_empty());
    assert_ne!(
        root, GENESIS_HASH,
        "root of 8-leaf tree should not be genesis"
    );

    // Verify inclusion proof for each audit record.
    for (i, record) in chain.iter().enumerate() {
        let proof = tree.proof(i);
        assert!(
            MerkleTree::verify_proof(&record.event_hash, &proof, &root),
            "audit record {i} should have valid Merkle proof"
        );
    }
}

// ===========================================================================
// Compliance Export Format Verification
// ===========================================================================

#[test]
fn test_compliance_export_soc2_format() {
    let records = make_chain(5);
    let exporter = Soc2Exporter;

    let output = exporter.export(&records).unwrap();
    let csv = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = csv.lines().collect();

    // Header + 5 data rows.
    assert_eq!(lines.len(), 6, "expected 1 header + 5 data rows");

    // Verify SOC2 headers.
    assert!(
        lines[0].starts_with("timestamp,event_type,instance_id,actor,action,resource,event_hash,prev_hash,signature_status"),
        "SOC2 header mismatch: {}",
        lines[0]
    );

    // Each data row should contain "verified" (all records are valid).
    for line in &lines[1..] {
        assert!(
            line.contains("verified"),
            "valid record should show 'verified', got: {line}"
        );
    }
}

#[test]
fn test_compliance_export_hipaa_format() {
    let records = make_chain(3);
    let exporter = HipaaExporter;

    let output = exporter.export(&records).unwrap();
    let csv = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = csv.lines().collect();

    assert_eq!(lines.len(), 4, "expected 1 header + 3 data rows");
    assert!(
        lines[0].contains("data_access_type"),
        "HIPAA header should contain data_access_type"
    );
    assert!(
        lines[0].contains("hash_verified"),
        "HIPAA header should contain hash_verified"
    );

    // All records should have hash_verified = true.
    for line in &lines[1..] {
        assert!(
            line.contains("true"),
            "valid record should show hash_verified=true, got: {line}"
        );
    }
}

#[test]
fn test_compliance_export_pci_format() {
    let records = make_chain(3);
    let exporter = PciExporter;

    let output = exporter.export(&records).unwrap();
    let csv = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = csv.lines().collect();

    assert_eq!(lines.len(), 4, "expected 1 header + 3 data rows");
    assert!(
        lines[0].contains("credential_access"),
        "PCI header should contain credential_access"
    );
}

#[test]
fn test_compliance_export_all_formats_consistent_row_count() {
    let records = make_chain(10);

    for format in [
        ComplianceFormat::Soc2,
        ComplianceFormat::Hipaa,
        ComplianceFormat::Pci,
    ] {
        let exporter = exporter_for(format);
        let output = exporter.export(&records).unwrap();
        let csv = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(
            lines.len(),
            11,
            "format {:?} should have 1 header + 10 data rows",
            format
        );
    }
}

#[test]
fn test_compliance_export_tampered_record_shows_invalid() {
    let mut records = make_chain(3);
    // Tamper with record 1's data — its hash should no longer verify.
    records[1].event_data = b"tampered".to_vec();

    let exporter = Soc2Exporter;
    let output = exporter.export(&records).unwrap();
    let csv = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = csv.lines().collect();

    // Record 0 and 2 should be "verified", record 1 should be "invalid".
    assert!(
        lines[1].contains("verified"),
        "untampered record 0 should be verified"
    );
    assert!(
        lines[2].contains("invalid"),
        "tampered record 1 should be invalid"
    );
}

// ===========================================================================
// Ed25519 Signatures + Audit Chain Integration
// ===========================================================================

#[test]
fn test_ed25519_sign_verify_with_audit_chain() {
    let signer = Ed25519Signer::generate();
    let chain = make_chain(5);

    // Sign each record's event_hash.
    let signatures: Vec<String> = chain
        .iter()
        .map(|record| signer.sign(record.event_hash.as_bytes()))
        .collect();

    // Verify all signatures.
    for (i, record) in chain.iter().enumerate() {
        assert!(
            signer.verify(record.event_hash.as_bytes(), &signatures[i]),
            "signature for record {i} should verify"
        );
    }
}

#[test]
fn test_ed25519_tampered_record_signature_fails() {
    let signer = Ed25519Signer::generate();
    let chain = make_chain(3);

    // Sign record 1.
    let signature = signer.sign(chain[1].event_hash.as_bytes());
    assert!(signer.verify(chain[1].event_hash.as_bytes(), &signature));

    // Tamper with the record — recompute hash with different data.
    let tampered_hash = compute_event_hash(b"tampered-data", &chain[0].event_hash);

    // Original signature should NOT verify against tampered hash.
    assert!(
        !signer.verify(tampered_hash.as_bytes(), &signature),
        "signature should fail for tampered record hash"
    );
}

#[test]
fn test_ed25519_different_signer_rejects() {
    let signer1 = Ed25519Signer::generate();
    let signer2 = Ed25519Signer::generate();
    let chain = make_chain(2);

    let sig = signer1.sign(chain[0].event_hash.as_bytes());
    assert!(
        !signer2.verify(chain[0].event_hash.as_bytes(), &sig),
        "different signer's key should not verify"
    );
}

#[test]
fn test_ed25519_sign_all_chain_records_and_verify_integrity() {
    // End-to-end: create chain, sign each record, verify both chain integrity
    // AND all signatures.
    let signer = Ed25519Signer::generate();
    let chain = make_chain(10);

    // Step 1: Verify hash chain integrity.
    assert!(verify_chain(&chain).is_ok(), "chain should be valid");

    // Step 2: Sign each record and verify.
    let signed_hashes: Vec<(String, String)> = chain
        .iter()
        .map(|r| {
            let sig = signer.sign(r.event_hash.as_bytes());
            (r.event_hash.clone(), sig)
        })
        .collect();

    for (i, (hash, sig)) in signed_hashes.iter().enumerate() {
        assert!(
            signer.verify(hash.as_bytes(), sig),
            "signature verification failed for record {i}"
        );
    }

    // Step 3: Verify that individual record hashes are also valid.
    for (i, record) in chain.iter().enumerate() {
        assert!(
            verify_record(record),
            "record {i} hash should be self-consistent"
        );
    }
}

// ===========================================================================
// Merkle + Signatures + Chain Combined (full audit pipeline)
// ===========================================================================

#[test]
fn test_full_audit_pipeline_chain_merkle_signatures() {
    let signer = Ed25519Signer::generate();
    let chain = make_chain(8);

    // 1. Verify hash chain.
    assert!(verify_chain(&chain).is_ok());

    // 2. Build Merkle tree from chain hashes.
    let leaf_hashes: Vec<String> = chain.iter().map(|r| r.event_hash.clone()).collect();
    let tree = MerkleTree::build(&leaf_hashes);
    let root = tree.root().to_string();

    // 3. Sign the Merkle root.
    let root_signature = signer.sign(root.as_bytes());
    assert!(signer.verify(root.as_bytes(), &root_signature));

    // 4. Verify each leaf has a valid Merkle proof.
    for (i, hash) in leaf_hashes.iter().enumerate() {
        let proof = tree.proof(i);
        assert!(
            MerkleTree::verify_proof(hash, &proof, &root),
            "Merkle proof failed for leaf {i}"
        );
    }

    // 5. Export as SOC2 — all records valid.
    let exporter = Soc2Exporter;
    let csv_bytes = exporter.export(&chain).unwrap();
    let csv = String::from_utf8(csv_bytes).unwrap();
    let data_lines = csv.lines().skip(1).count();
    assert_eq!(data_lines, 8, "SOC2 export should have 8 data rows");
}
