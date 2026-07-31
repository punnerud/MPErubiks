use kewb::DataTable;
use std::sync::OnceLock;

static TABLE: OnceLock<DataTable> = OnceLock::new();

/// Decode and install the solver table (6.8 MB bincode; ~8-10 MiB live).
/// Idempotent: a second call is a no-op.
pub fn install_table(bytes: &[u8]) -> Result<(), crate::SolveError> {
    if TABLE.get().is_some() {
        return Ok(());
    }
    let table =
        kewb::fs::decode_table(bytes).map_err(|e| crate::SolveError::Table(e.to_string()))?;
    let _ = TABLE.set(table);
    Ok(())
}

pub fn table_ready() -> bool {
    TABLE.get().is_some()
}

pub(crate) fn table() -> Result<&'static DataTable, crate::SolveError> {
    TABLE.get().ok_or(crate::SolveError::TableNotLoaded)
}
