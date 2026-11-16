//! The order `SORT` and `MERGE` put records in.
//!
//! Taken out of the interpreter unchanged (DESKTOP-WASM-001 T37, correction
//! C5), so that the native runner can order records exactly as the interpreter
//! does. The interpreter still works out each record's key values (it spreads
//! the record over the SD's fields and evaluates each key); this module orders
//! records by those values.

use std::cmp::Ordering;

use crate::collation::Collation;
use crate::value::CobolValue;

/// One record with its key values in key order, each with whether its key is
/// `ASCENDING`.
pub type KeyedRecord = (Vec<(CobolValue, bool)>, Vec<u8>);

/// Compare two records' key values, key by key, until one differs.
///
/// A named sequence (`COLLATING SEQUENCE IS alphabet-name`) orders
/// alphanumeric keys; numeric keys compare by value under any sequence. A
/// descending key reverses its own comparison only.
pub fn compare_keys(
    a: &[(CobolValue, bool)],
    b: &[(CobolValue, bool)],
    coll: Option<&Collation>,
) -> Ordering {
    for ((av, asc), (bv, _)) in a.iter().zip(b.iter()) {
        let ord = match (coll, av, bv) {
            (Some(c), CobolValue::String { .. }, CobolValue::String { .. }) => {
                c.compare(&av.as_display_string(), &bv.as_display_string())
            }
            _ => crate::interpreter::cob_ordering(av, bv),
        };
        if ord != Ordering::Equal {
            return if *asc { ord } else { ord.reverse() };
        }
    }
    Ordering::Equal
}

/// Stable-sort `records` by their key values. Records with equal keys keep the
/// order they came in, which is what `MERGE` (its inputs concatenated in
/// `USING` order) and `SORT … WITH DUPLICATES IN ORDER` rely on.
pub fn sort_keyed(records: &mut [KeyedRecord], coll: Option<&Collation>) {
    records.sort_by(|a, b| compare_keys(&a.0, &b.0, coll));
}
