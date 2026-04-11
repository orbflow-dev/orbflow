// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Generic pagination utility.

use crate::ports::ListOptions;

/// Applies offset/limit pagination to a slice.
///
/// A non-positive limit is treated as "no limit" (return everything from offset).
pub fn paginate<T: Clone>(all: &[T], opts: &ListOptions) -> Vec<T> {
    let len = all.len() as i64;
    let start = opts.offset.max(0).min(len);
    let end = if opts.limit <= 0 {
        len
    } else {
        (start + opts.limit).min(len)
    };
    all[start as usize..end as usize].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_pagination() {
        let items: Vec<i32> = (1..=10).collect();
        let result = paginate(
            &items,
            &ListOptions {
                offset: 2,
                limit: 3,
            },
        );
        assert_eq!(result, vec![3, 4, 5]);
    }

    #[test]
    fn test_no_limit() {
        let items: Vec<i32> = (1..=5).collect();
        let result = paginate(
            &items,
            &ListOptions {
                offset: 1,
                limit: 0,
            },
        );
        assert_eq!(result, vec![2, 3, 4, 5]);
    }

    #[test]
    fn test_offset_beyond_length() {
        let items: Vec<i32> = (1..=3).collect();
        let result = paginate(
            &items,
            &ListOptions {
                offset: 10,
                limit: 5,
            },
        );
        assert!(result.is_empty());
    }

    #[test]
    fn test_negative_offset() {
        let items: Vec<i32> = (1..=5).collect();
        let result = paginate(
            &items,
            &ListOptions {
                offset: -1,
                limit: 2,
            },
        );
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn test_limit_exceeds_remaining() {
        let items: Vec<i32> = (1..=5).collect();
        let result = paginate(
            &items,
            &ListOptions {
                offset: 3,
                limit: 100,
            },
        );
        assert_eq!(result, vec![4, 5]);
    }

    #[test]
    fn test_empty_slice() {
        let items: Vec<i32> = vec![];
        let result = paginate(
            &items,
            &ListOptions {
                offset: 0,
                limit: 10,
            },
        );
        assert!(result.is_empty());
    }
}
