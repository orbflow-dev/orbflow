// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Bounded result deduplication set with LRU eviction.

use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;

/// Maximum number of result IDs tracked per instance to prevent unbounded
/// memory growth.
const MAX_PROCESSED_RESULTS: usize = 100;

/// A bounded set of processed result IDs per instance. When the set exceeds
/// [`MAX_PROCESSED_RESULTS`], the oldest entry is evicted (FIFO) in O(1).
pub(crate) struct ResultSet {
    inner: Mutex<ResultSetInner>,
}

struct ResultSetInner {
    ids: HashSet<String>,
    order: VecDeque<String>,
}

impl ResultSet {
    /// Creates an empty result set.
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(ResultSetInner {
                ids: HashSet::with_capacity(MAX_PROCESSED_RESULTS),
                order: VecDeque::with_capacity(MAX_PROCESSED_RESULTS),
            }),
        }
    }

    /// Returns `true` if the given ID has already been processed.
    pub(crate) fn contains(&self, id: &str) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.ids.contains(id)
    }

    /// Records a result ID. If the set exceeds the maximum size, the oldest
    /// entry is evicted via O(1) `pop_front`.
    pub(crate) fn add(&self, id: String) {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if inner.ids.contains(&id) {
            return;
        }
        inner.ids.insert(id.clone());
        inner.order.push_back(id);
        if inner.order.len() > MAX_PROCESSED_RESULTS
            && let Some(evicted) = inner.order.pop_front()
        {
            inner.ids.remove(&evicted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_and_add() {
        let rs = ResultSet::new();
        assert!(!rs.contains("r1"));
        rs.add("r1".into());
        assert!(rs.contains("r1"));
    }

    #[test]
    fn test_duplicate_add_is_noop() {
        let rs = ResultSet::new();
        rs.add("r1".into());
        rs.add("r1".into());
        let inner = rs.inner.lock().unwrap();
        assert_eq!(inner.order.len(), 1);
    }

    #[test]
    fn test_eviction_on_overflow() {
        let rs = ResultSet::new();
        for i in 0..=MAX_PROCESSED_RESULTS {
            rs.add(format!("r{i}"));
        }
        // The first entry should have been evicted (FIFO via pop_front).
        assert!(!rs.contains("r0"));
        // The last entry should still be present.
        assert!(rs.contains(&format!("r{MAX_PROCESSED_RESULTS}")));
        let inner = rs.inner.lock().unwrap();
        assert_eq!(inner.order.len(), MAX_PROCESSED_RESULTS);
    }
}
