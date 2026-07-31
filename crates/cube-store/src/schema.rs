//! The database schema, expressed as an mpedb TOML config (the config IS
//! the schema in mpedb; attaching with a different config hard-errors,
//! which pins migrations to explicit code).

pub fn config_toml(path: &str, durability: &str, size_mb: u32) -> String {
    format!(
        r#"
[database]
path = "{path}"
size_mb = {size_mb}
max_readers = 4
durability = "{durability}"

[runtime]
max_query_threads = 1

[[table]]
name = "meta"
primary_key = ["k"]

  [[table.column]]
  name = "k"
  type = "text"

  [[table.column]]
  name = "v"
  type = "int64"
  nullable = false

[[table]]
name = "settings"
primary_key = ["key"]

  [[table.column]]
  name = "key"
  type = "text"

  [[table.column]]
  name = "value"
  type = "text"
  nullable = false

[[table]]
name = "algorithms"
primary_key = ["id"]

  [[table.column]]
  name = "id"
  type = "text"

  [[table.column]]
  name = "trained"
  type = "bool"
  nullable = false

[[table]]
name = "training_results"
primary_key = ["id"]

  [[table.column]]
  name = "id"
  type = "int64"

  [[table.column]]
  name = "alg_id"
  type = "text"
  nullable = false

  [[table.column]]
  name = "ts_ms"
  type = "int64"
  nullable = false

  [[table.column]]
  name = "duration_ms"
  type = "int64"
  nullable = false
  check = "duration_ms > 0"

  [[table.column]]
  name = "success"
  type = "bool"
  nullable = false

  [[table.index]]
  columns = ["alg_id", "ts_ms"]

  [[table.index]]
  columns = ["alg_id", "success", "duration_ms"]

[[table]]
name = "solutions"
primary_key = ["state"]

  [[table.column]]
  name = "state"
  type = "text"

  [[table.column]]
  name = "solution"
  type = "text"
  nullable = false

  [[table.column]]
  name = "move_count"
  type = "int64"
  nullable = false

  [[table.column]]
  name = "solve_ms"
  type = "int64"
  nullable = false

  [[table.column]]
  name = "hits"
  type = "int64"
  nullable = false
"#
    )
}

/// Tables in dump order (parents before children is irrelevant here — no
/// FKs — but keep it stable for readable dumps).
pub const TABLES: [(&str, &[&str]); 5] = [
    ("meta", &["k", "v"]),
    ("settings", &["key", "value"]),
    ("algorithms", &["id", "trained"]),
    (
        "training_results",
        &["id", "alg_id", "ts_ms", "duration_ms", "success"],
    ),
    (
        "solutions",
        &["state", "solution", "move_count", "solve_ms", "hits"],
    ),
];
