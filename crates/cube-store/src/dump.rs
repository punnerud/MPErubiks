//! Logical dump/restore: the browser persistence path (mpedb has no
//! filesystem in wasm). SELECT * of every table into JSON; restore
//! replays INSERTs into a fresh database.

use crate::{schema, Store, StoreError};
use mpedb::{ExecResult, Value};
use serde_json::json;

pub fn dump_json(store: &Store) -> Result<String, StoreError> {
    let mut tables = serde_json::Map::new();
    for (table, columns) in schema::TABLES {
        let sql = format!("SELECT {} FROM {table}", columns.join(", "));
        let mut rows_out = Vec::new();
        if let ExecResult::Rows { rows, .. } = store.db().query(&sql, &[])? {
            for row in rows {
                rows_out.push(
                    row.iter()
                        .map(value_to_json)
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
        }
        tables.insert(table.to_string(), json!(rows_out));
    }
    Ok(json!({ "version": 1, "tables": tables }).to_string())
}

pub fn restore_json(store: &Store, dump: &str) -> Result<(), StoreError> {
    let parsed: serde_json::Value =
        serde_json::from_str(dump).map_err(|e| StoreError(format!("bad dump: {e}")))?;
    if parsed["version"] != 1 {
        return Err(StoreError("unknown dump version".into()));
    }
    for (table, columns) in schema::TABLES {
        let Some(rows) = parsed["tables"][table].as_array() else {
            continue;
        };
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({})",
            columns.join(", "),
            placeholders.join(", ")
        );
        for row in rows {
            let Some(cells) = row.as_array() else {
                continue;
            };
            let values: Vec<Value> = cells.iter().map(json_to_value).collect();
            store.db().query(&sql, &values)?;
        }
    }
    Ok(())
}

fn value_to_json(v: &Value) -> Result<serde_json::Value, StoreError> {
    Ok(match v {
        Value::Null => serde_json::Value::Null,
        Value::Int(n) => json!(n),
        Value::Float(x) => json!(x),
        Value::Bool(b) => json!(b),
        Value::Text(s) => json!(s),
        Value::Timestamp(t) => json!({ "$ts": t }),
        Value::Blob(b) => json!({ "$b64": base64_encode(b) }),
        other => return Err(StoreError(format!("unsupported value in dump: {other:?}"))),
    })
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        serde_json::Value::Object(o) => {
            if let Some(t) = o.get("$ts").and_then(|t| t.as_i64()) {
                Value::Timestamp(t)
            } else if let Some(b) = o.get("$b64").and_then(|b| b.as_str()) {
                Value::Blob(base64_decode(b))
            } else {
                Value::Null
            }
        }
        serde_json::Value::Array(_) => Value::Null,
    }
}

// Tiny dependency-free base64 (blobs are not used by the app today, but
// the dump format should not silently corrupt them if they appear).
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(s: &str) -> Vec<u8> {
    let val = |c: u8| B64.iter().position(|&b| b == c).unwrap_or(0) as u32;
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c) << (18 - 6 * i);
        }
        let produced = chunk.len() * 6 / 8;
        for i in 0..produced {
            out.push(((n >> (16 - 8 * i)) & 0xFF) as u8);
        }
    }
    out
}
