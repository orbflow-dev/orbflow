// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Compile-and-behavior gate for T-08 Risk #1:
//!
//! `Zeroizing<Vec<u8>>` must deref transparently into `serde_json::from_slice`
//! so that the call-site wrap in `credential.rs` compiles and behaves correctly.
//!
//! This is a pure in-process test — no database, no network.
//! It guards the invariant: wrapping the plaintext buffer in `Zeroizing::new(...)`
//! does NOT prevent `serde_json::from_slice(&plaintext)` from parsing it, because
//! `Zeroizing<Vec<u8>>` implements `Deref<Target = Vec<u8>>` and `Vec<u8>`
//! coerces to `&[u8]` in a reference context.

use serde_json::Value;
use std::collections::HashMap;
use zeroize::Zeroizing;

/// T-08 Risk #1: Zeroizing<Vec<u8>> derefs into serde_json::from_slice without
/// re-allocating or losing bytes. A JSON object survives the round-trip.
#[test]
fn zeroizing_vec_derefs_into_from_slice_object() {
    let data: HashMap<&str, &str> = [("api_key", "s3cr3t"), ("region", "us-east-1")]
        .iter()
        .cloned()
        .collect();
    let bytes: Vec<u8> = serde_json::to_vec(&data).expect("serialize");

    // This is the pattern used in credential.rs:125:
    //   let plaintext = Zeroizing::new(crypto::decrypt(key, encrypted)?);
    //   serde_json::from_slice(&plaintext)
    let plaintext = Zeroizing::new(bytes);
    let parsed: HashMap<String, Value> =
        serde_json::from_slice(&plaintext).expect("parse must succeed via Deref");

    assert_eq!(parsed["api_key"], Value::String("s3cr3t".into()));
    assert_eq!(parsed["region"], Value::String("us-east-1".into()));
}

/// T-08 Risk #1 (edge): an empty JSON object `{}` also parses correctly
/// through the Zeroizing wrapper.
#[test]
fn zeroizing_vec_derefs_empty_object() {
    let empty_json = b"{}".to_vec();
    let plaintext = Zeroizing::new(empty_json);
    let parsed: HashMap<String, Value> =
        serde_json::from_slice(&plaintext).expect("empty object must parse");
    assert!(parsed.is_empty());
}

/// T-08 Risk #1 (edge): nested JSON values (strings, numbers, booleans) all
/// round-trip through Zeroizing without truncation or corruption.
#[test]
fn zeroizing_vec_derefs_nested_values() {
    let json = r#"{"token":"abc","ttl":3600,"active":true}"#;
    let plaintext = Zeroizing::new(json.as_bytes().to_vec());
    let parsed: HashMap<String, Value> =
        serde_json::from_slice(&plaintext).expect("nested values must parse");

    assert_eq!(parsed["token"], Value::String("abc".into()));
    assert_eq!(parsed["ttl"], Value::Number(3600.into()));
    assert_eq!(parsed["active"], Value::Bool(true));
}

/// T-08 scoping invariant: the Zeroizing wrapper covers ONLY the raw byte buffer.
/// After `from_slice`, the deserialized Value is NOT zeroized — this test
/// documents that boundary explicitly so future reviewers understand the scope.
///
/// Full secret-path zeroization (wrapping `Credential.data` itself) is T-14.
#[test]
fn zeroized_buffer_value_is_still_readable_after_parse() {
    let json = r#"{"password":"hunter2"}"#;
    let plaintext = Zeroizing::new(json.as_bytes().to_vec());

    // After from_slice the parsed Value lives in heap memory NOT covered by Zeroizing.
    // This is the documented residual from T-08 (see PLAN.md Risk #12).
    let parsed: HashMap<String, Value> = serde_json::from_slice(&plaintext).expect("parse");
    let password = parsed["password"].as_str().expect("string");
    assert_eq!(password, "hunter2");

    // Explicitly drop to trigger zeroization of the byte buffer.
    // The Value HashMap remains alive and readable — confirming T-08's label:
    // "buffer-only" zeroization, not full credential zeroization.
    drop(plaintext);
    assert_eq!(password, "hunter2"); // value unaffected by dropping the buffer
}
