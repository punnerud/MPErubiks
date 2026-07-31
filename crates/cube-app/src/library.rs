//! Loads the algorithm library (assets/algorithms.json, embedded) and
//! builds the recognizer. Bad data fails loudly at startup — the same
//! validation runs in cube-core's test suite, so this should never trip
//! in a released build.

use cube_core::{Alg, CaseDef, CaseSet, RecogKind, Recognizer};
use serde::Deserialize;

const LIBRARY_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/algorithms.json"
));

#[derive(Deserialize)]
struct LibraryFile {
    cases: Vec<CaseEntry>,
}

#[derive(Deserialize)]
struct CaseEntry {
    id: String,
    set: String,
    name: String,
    group: String,
    moves: String,
    recognition: String,
}

pub struct Library {
    pub rec: Recognizer,
}

impl Library {
    pub fn load() -> Library {
        let file: LibraryFile =
            serde_json::from_str(LIBRARY_JSON).expect("algorithms.json is valid JSON");
        let defs: Vec<CaseDef> = file
            .cases
            .into_iter()
            .map(|c| CaseDef {
                set: match c.set.as_str() {
                    "PLL" => CaseSet::Pll,
                    "OLL" => CaseSet::Oll,
                    "F2L" => CaseSet::F2l,
                    _ => CaseSet::Lbl,
                },
                recognition: match c.recognition.as_str() {
                    "oll" => RecogKind::Oll,
                    "pll" => RecogKind::Pll,
                    "f2l" => RecogKind::F2l,
                    _ => RecogKind::None,
                },
                alg: Alg::parse(&c.moves).expect("algorithms.json moves parse"),
                id: c.id,
                name: c.name,
                group: c.group,
            })
            .collect();
        Library {
            rec: Recognizer::new(defs).expect("algorithms.json passes validation"),
        }
    }

    pub fn cases_in_set(&self, set: CaseSet) -> Vec<u16> {
        (0..self.rec.defs().len() as u16)
            .filter(|&i| self.rec.case(i).set == set)
            .collect()
    }
}
