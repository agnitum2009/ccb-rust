//! Mirrors Python `lib/ccbrd/reload_transaction_records.py`.

use serde::Serialize;
use serde_json::Value;

/// Convert an optional value into a JSON record.
pub fn record<T: Serialize>(value: Option<T>) -> Option<Value> {
    value.and_then(|v| serde_json::to_value(v).ok())
}

/// Convert an optional rollback value into a JSON record.
pub fn rollback_record<T: Serialize>(value: Option<T>) -> Option<Value> {
    record(value)
}
