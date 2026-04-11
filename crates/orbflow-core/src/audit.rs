// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Immutable cryptographically-signed audit trail.
//!
//! Each event in the execution log is chained via SHA-256 hashes, forming a
//! tamper-evident log. If any event is modified or deleted, the hash chain
//! breaks and verification fails.
//!
//! # Hash Chain
//!
//! ```text
//! event_hash[0] = SHA-256(serialize(event[0]) || "genesis")
//! event_hash[n] = SHA-256(serialize(event[n]) || event_hash[n-1])
//! ```

use ed25519_dalek::{Signer, Verifier};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// An audit record wrapping a domain event with hash chain integrity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// The SHA-256 hash of the previous record (hex-encoded).
    /// For the first record in a chain, this is the genesis hash.
    pub prev_hash: String,
    /// The SHA-256 hash of this record (hex-encoded).
    /// Computed as: SHA-256(event_json || prev_hash).
    pub event_hash: String,
    /// The serialized event data (JSON bytes).
    pub event_data: Vec<u8>,
    /// Sequence number within the instance's event log.
    pub seq: u64,
}

/// The genesis hash used as prev_hash for the first event in a chain.
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Computes the SHA-256 hash for an audit record.
///
/// The hash is computed over: `len(event_data) || event_data || prev_hash_bytes`.
/// The length prefix prevents ambiguity between different (event_data, prev_hash)
/// pairs that could produce the same concatenated byte sequence.
pub fn compute_event_hash(event_data: &[u8], prev_hash: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update((event_data.len() as u64).to_le_bytes());
    hasher.update(event_data);
    hasher.update(prev_hash.as_bytes());
    hex_encode(hasher.finalize().as_slice())
}

/// Creates an audit record for an event, chaining to the previous hash.
pub fn create_audit_record(event_data: Vec<u8>, prev_hash: &str, seq: u64) -> AuditRecord {
    let event_hash = compute_event_hash(&event_data, prev_hash);
    AuditRecord {
        prev_hash: prev_hash.to_string(),
        event_hash,
        event_data,
        seq,
    }
}

/// Verifies the integrity of a single audit record.
///
/// Returns `true` if the record's `event_hash` matches the recomputed hash.
pub fn verify_record(record: &AuditRecord) -> bool {
    let expected = compute_event_hash(&record.event_data, &record.prev_hash);
    record.event_hash == expected
}

/// Verifies the integrity of a chain of audit records.
///
/// Returns `Ok(())` if all records are valid and properly chained.
/// Returns `Err` with the index of the first invalid record.
pub fn verify_chain(records: &[AuditRecord]) -> Result<(), AuditChainError> {
    if records.is_empty() {
        return Ok(());
    }

    // Verify first record links to genesis.
    if records[0].prev_hash != GENESIS_HASH {
        return Err(AuditChainError::InvalidGenesis);
    }

    for (i, record) in records.iter().enumerate() {
        // Verify the hash of each record.
        if !verify_record(record) {
            return Err(AuditChainError::TamperedRecord { index: i });
        }

        // Verify chain linkage (each record's prev_hash == previous record's event_hash).
        if i > 0 && record.prev_hash != records[i - 1].event_hash {
            return Err(AuditChainError::BrokenChain { index: i });
        }

        // Verify sequence numbers are monotonically increasing.
        if record.seq != i as u64 {
            return Err(AuditChainError::InvalidSequence {
                index: i,
                expected: i as u64,
                actual: record.seq,
            });
        }
    }

    Ok(())
}

/// Errors from audit chain verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditChainError {
    /// The first record does not link to the genesis hash.
    InvalidGenesis,
    /// A record's hash does not match its content.
    TamperedRecord { index: usize },
    /// A record's prev_hash does not match the previous record's event_hash.
    BrokenChain { index: usize },
    /// Sequence numbers are not monotonically increasing.
    InvalidSequence {
        index: usize,
        expected: u64,
        actual: u64,
    },
}

impl std::fmt::Display for AuditChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGenesis => write!(f, "first record does not link to genesis hash"),
            Self::TamperedRecord { index } => write!(f, "record {index} has been tampered with"),
            Self::BrokenChain { index } => write!(f, "chain broken at record {index}"),
            Self::InvalidSequence {
                index,
                expected,
                actual,
            } => write!(f, "record {index}: expected seq {expected}, got {actual}"),
        }
    }
}

impl std::error::Error for AuditChainError {}

// ─── Digital Signatures ─────────────────────────────────────────────────────

/// Trait for signing audit events (optional, for regulated environments).
pub trait AuditSigner: Send + Sync {
    /// Sign the given data and return the signature as a hex-encoded string.
    fn sign(&self, data: &[u8]) -> String;
    /// Verify a hex-encoded signature against the given data.
    fn verify(&self, data: &[u8], signature: &str) -> bool;
}

/// Ed25519 digital signature implementation for audit event signing.
pub struct Ed25519Signer {
    signing_key: ed25519_dalek::SigningKey,
    verifying_key: ed25519_dalek::VerifyingKey,
}

impl Ed25519Signer {
    /// Generate a new Ed25519 keypair using a secure random source.
    pub fn generate() -> Self {
        let secret: [u8; 32] = rand::rng().random();
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Reconstruct an Ed25519 signer from raw keypair bytes.
    ///
    /// `secret` must be exactly 32 bytes (the Ed25519 secret key seed).
    /// `public` must be exactly 32 bytes (the Ed25519 public key).
    pub fn from_keypair(secret: &[u8], public: &[u8]) -> Result<Self, String> {
        let secret_bytes: [u8; 32] = secret
            .try_into()
            .map_err(|_| format!("secret key must be 32 bytes, got {}", secret.len()))?;
        let public_bytes: [u8; 32] = public
            .try_into()
            .map_err(|_| format!("public key must be 32 bytes, got {}", public.len()))?;

        let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_bytes);
        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&public_bytes)
            .map_err(|e| format!("invalid public key: {e}"))?;

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Returns the raw public key bytes (32 bytes).
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.verifying_key.to_bytes().to_vec()
    }
}

impl AuditSigner for Ed25519Signer {
    fn sign(&self, data: &[u8]) -> String {
        let signature = self.signing_key.sign(data);
        hex_encode(&signature.to_bytes())
    }

    fn verify(&self, data: &[u8], signature: &str) -> bool {
        let sig_bytes = match hex_decode(signature) {
            Some(b) => b,
            None => return false,
        };
        let sig_array: [u8; 64] = match sig_bytes.try_into() {
            Ok(a) => a,
            Err(_) => return false,
        };
        let sig = ed25519_dalek::Signature::from_bytes(&sig_array);
        self.verifying_key.verify(data, &sig).is_ok()
    }
}

// ─── Merkle Tree ────────────────────────────────────────────────────────────

/// Position of a sibling node in a Merkle proof step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MerklePosition {
    Left,
    Right,
}

/// A single node in a Merkle inclusion proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProofNode {
    pub hash: String,
    pub position: MerklePosition,
}

/// A Merkle tree built from SHA-256 hashes.
///
/// `layers[0]` contains the leaf hashes, and `layers[last]` contains the root.
/// For an empty tree the root is the zero hash. For a single leaf the root
/// equals that leaf.
pub struct MerkleTree {
    layers: Vec<Vec<String>>,
}

impl MerkleTree {
    /// Build a Merkle tree from a slice of hex-encoded leaf hashes.
    pub fn build(hashes: &[String]) -> Self {
        if hashes.is_empty() {
            return Self {
                layers: vec![vec![GENESIS_HASH.to_string()]],
            };
        }

        let mut layers: Vec<Vec<String>> = vec![hashes.to_vec()];

        let mut current = hashes.to_vec();
        while current.len() > 1 {
            let mut next = Vec::with_capacity(current.len().div_ceil(2));
            for pair in current.chunks(2) {
                if pair.len() == 2 {
                    next.push(hash_pair(&pair[0], &pair[1]));
                } else {
                    // Odd element — duplicate it.
                    next.push(hash_pair(&pair[0], &pair[0]));
                }
            }
            layers.push(next.clone());
            current = next;
        }

        Self { layers }
    }

    /// Returns the Merkle root hash.
    pub fn root(&self) -> &str {
        self.layers
            .last()
            .and_then(|l| l.first())
            .map(String::as_str)
            .unwrap_or(GENESIS_HASH)
    }

    /// Generates an inclusion proof for the leaf at `index`.
    ///
    /// Returns an empty proof if the index is out of bounds.
    pub fn proof(&self, index: usize) -> Vec<MerkleProofNode> {
        if self.layers.is_empty() || index >= self.layers[0].len() {
            return Vec::new();
        }
        // Single leaf — no siblings needed.
        if self.layers[0].len() == 1 {
            return Vec::new();
        }

        let mut proof = Vec::new();
        let mut idx = index;

        // Walk from leaves up to (but not including) the root layer.
        for layer in &self.layers[..self.layers.len() - 1] {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let sibling_hash = if sibling_idx < layer.len() {
                layer[sibling_idx].clone()
            } else {
                // Odd layer — the element is duplicated.
                layer[idx].clone()
            };
            let position = if idx % 2 == 0 {
                MerklePosition::Right
            } else {
                MerklePosition::Left
            };
            proof.push(MerkleProofNode {
                hash: sibling_hash,
                position,
            });
            idx /= 2;
        }

        proof
    }

    /// Verifies an inclusion proof for a leaf against a known root.
    pub fn verify_proof(leaf: &str, proof: &[MerkleProofNode], root: &str) -> bool {
        let mut current = leaf.to_string();
        for node in proof {
            current = match node.position {
                MerklePosition::Left => hash_pair(&node.hash, &current),
                MerklePosition::Right => hash_pair(&current, &node.hash),
            };
        }
        current == root
    }
}

/// Hashes two hex-encoded hashes together: SHA-256(left_bytes || right_bytes).
fn hash_pair(left: &str, right: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    hex_encode(hasher.finalize().as_slice())
}

/// Hex-encodes a byte slice (lowercase).
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex-decodes a string into bytes. Returns `None` on invalid input.
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chain(n: usize) -> Vec<AuditRecord> {
        let mut records = Vec::new();
        let mut prev_hash = GENESIS_HASH.to_string();

        for i in 0..n {
            let data = format!(r#"{{"event":"test","seq":{i}}}"#).into_bytes();
            let record = create_audit_record(data, &prev_hash, i as u64);
            prev_hash = record.event_hash.clone();
            records.push(record);
        }

        records
    }

    #[test]
    fn test_single_record() {
        let data = b"hello world".to_vec();
        let record = create_audit_record(data, GENESIS_HASH, 0);

        assert_eq!(record.prev_hash, GENESIS_HASH);
        assert_eq!(record.seq, 0);
        assert!(!record.event_hash.is_empty());
        assert!(verify_record(&record));
    }

    #[test]
    fn test_chain_integrity() {
        let chain = make_chain(5);
        assert!(verify_chain(&chain).is_ok());
    }

    #[test]
    fn test_tampered_data() {
        let mut chain = make_chain(3);
        // Tamper with the second record's data.
        chain[1].event_data = b"tampered".to_vec();

        let err = verify_chain(&chain).unwrap_err();
        assert_eq!(err, AuditChainError::TamperedRecord { index: 1 });
    }

    #[test]
    fn test_broken_chain() {
        let mut chain = make_chain(3);
        // Break the chain by changing prev_hash.
        chain[2].prev_hash = "bad_hash".into();
        // Also recompute the event_hash so it matches the new prev_hash
        // (simulating an attempt to rewrite history).
        chain[2].event_hash = compute_event_hash(&chain[2].event_data, &chain[2].prev_hash);

        let err = verify_chain(&chain).unwrap_err();
        assert_eq!(err, AuditChainError::BrokenChain { index: 2 });
    }

    #[test]
    fn test_invalid_genesis() {
        let mut chain = make_chain(2);
        chain[0].prev_hash = "not_genesis".into();
        chain[0].event_hash = compute_event_hash(&chain[0].event_data, &chain[0].prev_hash);

        let err = verify_chain(&chain).unwrap_err();
        assert_eq!(err, AuditChainError::InvalidGenesis);
    }

    #[test]
    fn test_empty_chain_is_valid() {
        assert!(verify_chain(&[]).is_ok());
    }

    #[test]
    fn test_deterministic_hash() {
        let data = b"test data".to_vec();
        let h1 = compute_event_hash(&data, GENESIS_HASH);
        let h2 = compute_event_hash(&data, GENESIS_HASH);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_record_serde_roundtrip() {
        let record = create_audit_record(b"test".to_vec(), GENESIS_HASH, 0);
        let json = serde_json::to_string(&record).unwrap();
        let record2: AuditRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.event_hash, record2.event_hash);
        assert_eq!(record.prev_hash, record2.prev_hash);
        assert_eq!(record.seq, record2.seq);
    }

    // ─── Ed25519 Signer Tests ───────────────────────────────────────────────

    #[test]
    fn test_ed25519_sign_verify_roundtrip() {
        let signer = Ed25519Signer::generate();
        let data = b"audit event payload";
        let signature = signer.sign(data);
        assert!(signer.verify(data, &signature));
    }

    #[test]
    fn test_ed25519_tampered_signature_rejected() {
        let signer = Ed25519Signer::generate();
        let data = b"original data";
        let signature = signer.sign(data);

        // Tamper with the data.
        let tampered = b"tampered data";
        assert!(!signer.verify(tampered, &signature));
    }

    #[test]
    fn test_ed25519_invalid_signature_format() {
        let signer = Ed25519Signer::generate();
        assert!(!signer.verify(b"data", "not_hex"));
        assert!(!signer.verify(b"data", "abcd")); // too short
        assert!(!signer.verify(b"data", "")); // empty
    }

    #[test]
    fn test_ed25519_from_keypair_roundtrip() {
        let original = Ed25519Signer::generate();
        let secret = original.signing_key.to_bytes();
        let public = original.public_key_bytes();

        let restored = Ed25519Signer::from_keypair(&secret, &public).unwrap();
        let data = b"test roundtrip";
        let sig = original.sign(data);
        assert!(restored.verify(data, &sig));
    }

    #[test]
    fn test_ed25519_from_keypair_invalid_lengths() {
        assert!(Ed25519Signer::from_keypair(&[0u8; 16], &[0u8; 32]).is_err());
        assert!(Ed25519Signer::from_keypair(&[0u8; 32], &[0u8; 16]).is_err());
    }

    #[test]
    fn test_ed25519_different_keys_reject() {
        let signer1 = Ed25519Signer::generate();
        let signer2 = Ed25519Signer::generate();
        let data = b"signed by signer1";
        let sig = signer1.sign(data);
        assert!(!signer2.verify(data, &sig));
    }

    // ─── Merkle Tree Tests ──────────────────────────────────────────────────

    #[test]
    fn test_merkle_empty_tree() {
        let tree = MerkleTree::build(&[]);
        assert_eq!(tree.root(), GENESIS_HASH);
        assert!(tree.proof(0).is_empty());
    }

    #[test]
    fn test_merkle_single_leaf() {
        let leaf = compute_event_hash(b"event0", GENESIS_HASH);
        let tree = MerkleTree::build(&[leaf.clone()]);
        assert_eq!(tree.root(), leaf.as_str());
        // Single leaf needs no proof nodes.
        assert!(tree.proof(0).is_empty());
        assert!(MerkleTree::verify_proof(&leaf, &[], &leaf));
    }

    #[test]
    fn test_merkle_two_leaves() {
        let h0 = compute_event_hash(b"event0", GENESIS_HASH);
        let h1 = compute_event_hash(b"event1", GENESIS_HASH);
        let tree = MerkleTree::build(&[h0.clone(), h1.clone()]);

        let expected_root = hash_pair(&h0, &h1);
        assert_eq!(tree.root(), expected_root.as_str());

        // Proof for leaf 0.
        let proof0 = tree.proof(0);
        assert_eq!(proof0.len(), 1);
        assert!(MerkleTree::verify_proof(&h0, &proof0, tree.root()));

        // Proof for leaf 1.
        let proof1 = tree.proof(1);
        assert_eq!(proof1.len(), 1);
        assert!(MerkleTree::verify_proof(&h1, &proof1, tree.root()));
    }

    #[test]
    fn test_merkle_four_leaves() {
        let hashes: Vec<String> = (0..4)
            .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
            .collect();
        let tree = MerkleTree::build(&hashes);

        for (i, h) in hashes.iter().enumerate() {
            let proof = tree.proof(i);
            assert!(
                MerkleTree::verify_proof(h, &proof, tree.root()),
                "proof failed for leaf {i}"
            );
        }
    }

    #[test]
    fn test_merkle_odd_number_of_leaves() {
        let hashes: Vec<String> = (0..5)
            .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
            .collect();
        let tree = MerkleTree::build(&hashes);

        for (i, h) in hashes.iter().enumerate() {
            let proof = tree.proof(i);
            assert!(
                MerkleTree::verify_proof(h, &proof, tree.root()),
                "proof failed for leaf {i}"
            );
        }
    }

    #[test]
    fn test_merkle_invalid_proof_rejected() {
        let hashes: Vec<String> = (0..4)
            .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
            .collect();
        let tree = MerkleTree::build(&hashes);

        // Use a valid proof for leaf 0 but verify against leaf 1 — should fail.
        let proof0 = tree.proof(0);
        assert!(!MerkleTree::verify_proof(&hashes[1], &proof0, tree.root()));
    }

    #[test]
    fn test_merkle_tampered_root_rejected() {
        let hashes: Vec<String> = (0..3)
            .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
            .collect();
        let tree = MerkleTree::build(&hashes);
        let proof = tree.proof(0);

        let fake_root = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        assert!(!MerkleTree::verify_proof(&hashes[0], &proof, fake_root));
    }

    #[test]
    fn test_merkle_out_of_bounds_proof() {
        let hashes: Vec<String> = (0..3)
            .map(|i| compute_event_hash(format!("event{i}").as_bytes(), GENESIS_HASH))
            .collect();
        let tree = MerkleTree::build(&hashes);
        assert!(tree.proof(99).is_empty());
    }
}
