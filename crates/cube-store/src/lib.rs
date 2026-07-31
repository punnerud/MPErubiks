//! Storage layer on MPEdb (Morten's embedded database).
//!
//! Native: file-backed with WAL durability. Browser: the same engine
//! compiled to wasm runs `:memory:`-only, so persistence is a logical
//! dump (JSON of every table) that the app writes to localStorage and
//! replays on startup — see `dump_json` / `restore_json`.
//!
//! The schema lives in the TOML config (mpedb treats the config as
//! schema-authoritative), so opens are idempotent on both platforms.

mod dump;
mod schema;

pub use dump::{dump_json, restore_json};

use mpedb::{params, Database, ExecResult, Value};
use std::fmt;

pub struct Store {
    db: Database,
}

#[derive(Debug)]
pub struct StoreError(pub String);

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "store: {}", self.0)
    }
}

impl std::error::Error for StoreError {}

impl From<mpedb::Error> for StoreError {
    fn from(e: mpedb::Error) -> Self {
        StoreError(e.to_string())
    }
}

type Result<T> = std::result::Result<T, StoreError>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AlgStats {
    pub attempts: u64,
    pub successes: u64,
    pub best_ms: Option<i64>,
    /// Mean of the last (up to) 5 successful times.
    pub avg5_ms: Option<i64>,
}

impl Store {
    /// Open the file-backed store (native platforms).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_file(path: &std::path::Path) -> Result<Store> {
        let config = schema::config_toml(&path.display().to_string(), "wal", 32);
        let db = Database::open_with_config(
            mpedb::Config::from_toml_str(&config).map_err(|e| StoreError(e.to_string()))?,
        )?;
        Ok(Store { db })
    }

    /// Open the in-memory store (browser; also used by tests).
    pub fn open_in_memory() -> Result<Store> {
        let config = schema::config_toml(":memory:", "none", 4);
        let db = Database::open_with_config(
            mpedb::Config::from_toml_str(&config).map_err(|e| StoreError(e.to_string()))?,
        )?;
        Ok(Store { db })
    }

    pub(crate) fn db(&self) -> &Database {
        &self.db
    }

    /// Record a training attempt; returns the row id.
    pub fn record_result(
        &self,
        alg_id: &str,
        ts_ms: i64,
        duration_ms: i64,
        success: bool,
    ) -> Result<i64> {
        let id = self.next_id("next_result_id")?;
        self.db.query(
            "INSERT INTO training_results (id, alg_id, ts_ms, duration_ms, success) \
             VALUES ($1, $2, $3, $4, $5)",
            &params![id, alg_id, ts_ms, duration_ms, success],
        )?;
        Ok(id)
    }

    pub fn stats(&self, alg_id: &str) -> Result<AlgStats> {
        let mut out = AlgStats::default();
        if let ExecResult::Rows { rows, .. } = self.db.query(
            "SELECT COUNT(*) FROM training_results WHERE alg_id = $1",
            &params![alg_id],
        )? {
            out.attempts = first_int(&rows).unwrap_or(0) as u64;
        }
        if let ExecResult::Rows { rows, .. } = self.db.query(
            "SELECT COUNT(*), MIN(duration_ms) FROM training_results \
             WHERE alg_id = $1 AND success = $2",
            &params![alg_id, true],
        )? {
            if let Some(row) = rows.first() {
                if let Some(Value::Int(n)) = row.first() {
                    out.successes = *n as u64;
                }
                if let Some(Value::Int(min)) = row.get(1) {
                    out.best_ms = Some(*min);
                }
            }
        }
        if let ExecResult::Rows { rows, .. } = self.db.query(
            "SELECT duration_ms FROM training_results \
             WHERE alg_id = $1 AND success = $2 ORDER BY ts_ms DESC LIMIT 5",
            &params![alg_id, true],
        )? {
            let times: Vec<i64> = rows
                .iter()
                .filter_map(|r| match r.first() {
                    Some(Value::Int(n)) => Some(*n),
                    _ => None,
                })
                .collect();
            if !times.is_empty() {
                out.avg5_ms = Some(times.iter().sum::<i64>() / times.len() as i64);
            }
        }
        Ok(out)
    }

    pub fn set_trained(&self, alg_id: &str, trained: bool) -> Result<()> {
        self.db.query(
            "INSERT INTO algorithms (id, trained) VALUES ($1, $2) \
             ON CONFLICT (id) DO UPDATE SET trained = $2",
            &params![alg_id, trained],
        )?;
        Ok(())
    }

    pub fn trained_ids(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        if let ExecResult::Rows { rows, .. } = self.db.query(
            "SELECT id FROM algorithms WHERE trained = $1",
            &params![true],
        )? {
            for row in rows {
                if let Some(Value::Text(id)) = row.into_iter().next() {
                    out.push(id);
                }
            }
        }
        Ok(out)
    }

    /// Solution cache: the MPEE matrix-broker pattern — expensive results
    /// (kewb searches) bought once, cached locally, free on repeat. Keyed
    /// by the orientation-normalized facelet string.
    pub fn cached_solution(&self, state: &str) -> Result<Option<String>> {
        if let ExecResult::Rows { rows, .. } = self.db.query(
            "SELECT solution FROM solutions WHERE state = $1",
            &params![state],
        )? {
            for row in rows {
                if let Some(Value::Text(sol)) = row.into_iter().next() {
                    let _ = self.db.query(
                        "UPDATE solutions SET hits = hits + 1 WHERE state = $1",
                        &params![state],
                    );
                    return Ok(Some(sol));
                }
            }
        }
        Ok(None)
    }

    pub fn put_solution(
        &self,
        state: &str,
        solution: &str,
        move_count: i64,
        solve_ms: i64,
    ) -> Result<()> {
        self.db.query(
            "INSERT INTO solutions (state, solution, move_count, solve_ms, hits) \
             VALUES ($1, $2, $3, $4, 1) \
             ON CONFLICT (state) DO UPDATE SET solution = $2, move_count = $3",
            &params![state, solution, move_count, solve_ms],
        )?;
        Ok(())
    }

    /// Every recorded attempt as `(alg_id, duration_ms, success)`,
    /// chronological — used to warm the UI's stats at startup.
    pub fn all_results(&self) -> Result<Vec<(String, i64, bool)>> {
        let mut out = Vec::new();
        if let ExecResult::Rows { rows, .. } = self.db.query(
            "SELECT alg_id, duration_ms, success FROM training_results ORDER BY ts_ms",
            &[],
        )? {
            for row in rows {
                let mut it = row.into_iter();
                if let (Some(Value::Text(id)), Some(Value::Int(ms)), Some(Value::Bool(ok))) =
                    (it.next(), it.next(), it.next())
                {
                    out.push((id, ms, ok));
                }
            }
        }
        Ok(out)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        if let ExecResult::Rows { rows, .. } = self.db.query(
            "SELECT value FROM settings WHERE key = $1",
            &params![key],
        )? {
            for row in rows {
                if let Some(Value::Text(v)) = row.into_iter().next() {
                    return Ok(Some(v));
                }
            }
        }
        Ok(None)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.db.query(
            "INSERT INTO settings (key, value) VALUES ($1, $2) \
             ON CONFLICT (key) DO UPDATE SET value = $2",
            &params![key, value],
        )?;
        Ok(())
    }

    /// Monotonic id from the meta counter (mpedb has no last_insert_rowid;
    /// upsert-returning is the supported pattern).
    fn next_id(&self, counter: &str) -> Result<i64> {
        let result = self.db.query(
            "INSERT INTO meta (k, v) VALUES ($1, 1) \
             ON CONFLICT (k) DO UPDATE SET v = v + 1 RETURNING v",
            &params![counter],
        )?;
        if let ExecResult::Rows { rows, .. } = result {
            if let Some(v) = first_int(&rows) {
                return Ok(v);
            }
        }
        Err(StoreError("id counter returned no row".into()))
    }
}

fn first_int(rows: &[Vec<Value>]) -> Option<i64> {
    match rows.first()?.first()? {
        Value::Int(n) => Some(*n),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_stats_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.stats("pll-t").unwrap(), AlgStats::default());
        let id1 = store.record_result("pll-t", 1000, 2500, true).unwrap();
        let id2 = store.record_result("pll-t", 2000, 2100, true).unwrap();
        let _ = store.record_result("pll-t", 3000, 9999, false).unwrap();
        let _ = store.record_result("oll-27", 4000, 3000, true).unwrap();
        assert!(id2 > id1);

        let s = store.stats("pll-t").unwrap();
        assert_eq!(s.attempts, 3);
        assert_eq!(s.successes, 2);
        assert_eq!(s.best_ms, Some(2100));
        assert_eq!(s.avg5_ms, Some(2300));
    }

    #[test]
    fn trained_flags_and_settings() {
        let store = Store::open_in_memory().unwrap();
        store.set_trained("pll-t", true).unwrap();
        store.set_trained("oll-27", true).unwrap();
        store.set_trained("pll-t", false).unwrap();
        assert_eq!(store.trained_ids().unwrap(), vec!["oll-27".to_string()]);

        assert_eq!(store.setting("lang").unwrap(), None);
        store.set_setting("lang", "no").unwrap();
        store.set_setting("lang", "en").unwrap();
        assert_eq!(store.setting("lang").unwrap(), Some("en".into()));
    }

    #[test]
    fn dump_restore_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        store.record_result("pll-t", 1000, 2500, true).unwrap();
        store.record_result("oll-27", 2000, 1800, false).unwrap();
        store.set_trained("pll-t", true).unwrap();
        store.set_setting("lang", "no").unwrap();
        let json = dump_json(&store).unwrap();

        let fresh = Store::open_in_memory().unwrap();
        restore_json(&fresh, &json).unwrap();
        assert_eq!(fresh.stats("pll-t").unwrap(), store.stats("pll-t").unwrap());
        assert_eq!(fresh.trained_ids().unwrap(), store.trained_ids().unwrap());
        assert_eq!(fresh.setting("lang").unwrap(), Some("no".into()));
        // The id counter continues past restored rows.
        let next = fresh.record_result("pll-t", 3000, 2000, true).unwrap();
        assert!(next >= 3, "id counter must continue, got {next}");
    }

    #[test]
    fn solution_cache_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        let key = "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB";
        assert_eq!(store.cached_solution(key).unwrap(), None);
        store.put_solution(key, "R U R' U'", 4, 1500).unwrap();
        assert_eq!(
            store.cached_solution(key).unwrap(),
            Some("R U R' U'".to_string())
        );
        // Upsert replaces; hit counting doesn't corrupt the row.
        store.put_solution(key, "F2", 1, 100).unwrap();
        assert_eq!(store.cached_solution(key).unwrap(), Some("F2".to_string()));
        // The cache survives the wasm persistence path.
        let json = dump_json(&store).unwrap();
        let fresh = Store::open_in_memory().unwrap();
        restore_json(&fresh, &json).unwrap();
        assert_eq!(fresh.cached_solution(key).unwrap(), Some("F2".to_string()));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn file_backed_reopen() {
        let dir = std::env::temp_dir().join(format!("rubiks-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.mpedb");
        let _ = std::fs::remove_file(&path);
        {
            let store = Store::open_file(&path).unwrap();
            store.record_result("pll-t", 1000, 2500, true).unwrap();
        }
        {
            let store = Store::open_file(&path).unwrap();
            let s = store.stats("pll-t").unwrap();
            assert_eq!(s.attempts, 1);
        }
        let _ = std::fs::remove_file(&path);
    }
}
