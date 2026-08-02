//! App-side persistence glue over cube-store: opening the right backend
//! per platform, warming app state at startup, and (on wasm) mirroring
//! every mutation into localStorage as a logical dump.

use crate::app::RubiksApp;
use crate::screens::train::Attempt;
use cube_store::Store;

const WASM_DUMP_KEY: &str = "rubiks.db.v1";

#[cfg(not(target_arch = "wasm32"))]
pub fn open_store() -> Option<Store> {
    use std::path::PathBuf;
    let dir = std::env::var_os("RUBIKS_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/rubiks"))
        })?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::error!("store dir: {e}");
        return None;
    }
    match Store::open_file(&dir.join("rubiks.mpedb")) {
        Ok(s) => Some(s),
        Err(e) => {
            log::error!("store open: {e}");
            None
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn open_store() -> Option<Store> {
    let store = match Store::open_in_memory() {
        Ok(s) => s,
        Err(e) => {
            log::error!("store open: {e}");
            return None;
        }
    };
    if let Some(json) = local_storage().and_then(|ls| ls.get_item(WASM_DUMP_KEY).ok().flatten()) {
        if let Err(e) = cube_store::restore_json(&store, &json) {
            log::error!("store restore: {e} — starting fresh");
        }
    }
    Some(store)
}

/// On wasm every mutation is mirrored to localStorage (the data is tiny —
/// a full dump is cheaper than being clever). Native WAL persists itself.
pub fn persist(store: &Store) {
    #[cfg(target_arch = "wasm32")]
    {
        match cube_store::dump_json(store) {
            Ok(json) => {
                if let Some(ls) = local_storage() {
                    let _ = ls.set_item(WASM_DUMP_KEY, &json);
                }
            }
            Err(e) => log::error!("store dump: {e}"),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = store;
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Load persisted state into the app (trained flags, attempts, language).
pub fn warm_app(app: &mut RubiksApp) {
    let Some(store) = &app.store else { return };
    if let Ok(ids) = store.trained_ids() {
        for id in ids {
            if let Some(idx) = app.library.rec.find_by_id(&id) {
                app.trained.insert(idx);
            }
        }
    }
    // Guided-solution prefs — AFTER trained, so unknown/untrained ids
    // can be dropped and newly trained ones appended (lowest priority).
    {
        let mode = store.setting("hints.mode").ok().flatten();
        app.hints.mode = match mode.as_deref() {
            Some("practice") => crate::app::HintMode::Practice,
            Some("off") => crate::app::HintMode::Off,
            _ => crate::app::HintMode::Full,
        };
        if let Some(v) = store.setting("hints.max_extra").ok().flatten() {
            if let Ok(n) = v.parse::<usize>() {
                app.hints.max_extra = n.min(12);
            }
        }
        let mut include: Vec<u16> = Vec::new();
        if let Some(ids) = store.setting("hints.include").ok().flatten() {
            for id in ids.split(',').filter(|s| !s.is_empty()) {
                if let Some(idx) = app.library.rec.find_by_id(id) {
                    if app.trained.contains(&idx) && !include.contains(&idx) {
                        include.push(idx);
                    }
                }
            }
        } else {
            // First run: every trained algorithm included, library order.
            let mut t: Vec<u16> = app.trained.iter().copied().collect();
            t.sort_unstable();
            include = t;
        }
        for &t in &app.trained {
            if !include.contains(&t) {
                include.push(t);
            }
        }
        app.hints.include = include;
        // Play practice-scramble prefs.
        if let Some(ids) = store.setting("play.include").ok().flatten() {
            app.play.include = ids
                .split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|id| app.library.rec.find_by_id(id))
                .collect();
        }
        if let Some(v) = store.setting("play.target").ok().flatten() {
            if let Ok(n) = v.parse::<usize>() {
                app.play.target = n.clamp(1, 3);
            }
        }
        app.play.mode = match store.setting("play.mode").ok().flatten().as_deref() {
            Some("random") => cube_solver::ScrambleMode::Random,
            Some("built") => cube_solver::ScrambleMode::Built,
            _ => cube_solver::ScrambleMode::Auto,
        };
        app.tap_cell = store
            .setting("tap_select")
            .ok()
            .flatten()
            .is_some_and(|v| v == "cell");
    }
    if let Ok(rows) = store.all_results() {
        for (id, ms, success) in rows {
            if let Some(idx) = app.library.rec.find_by_id(&id) {
                app.attempts.entry(idx).or_default().push(Attempt {
                    ms: ms.max(0) as u64,
                    success,
                });
            }
        }
    }
    if let Ok(Some(lang)) = store.setting("lang") {
        app.i18n.lang = match lang.as_str() {
            code => crate::i18n::Lang::from_code(code).unwrap_or(crate::i18n::Lang::EN),
        };
    }
}

pub fn now_ms() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
}
