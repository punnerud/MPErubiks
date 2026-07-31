//! Validation of the real algorithm library (assets/algorithms.json).
//!
//! Every case is checked mechanically: the recognizer derives its pattern
//! from the algorithm's inverse, so a wrong-stage alg, a collision between
//! cases, or a typo that changes the case class fails here — data entry is
//! safe to do at scale.

use cube_core::{Alg, CaseDef, CaseSet, FaceletCube, RecogKind, Recognizer, SplitMix64};
use serde::Deserialize;

const LIBRARY_JSON: &str = include_str!("../../../assets/algorithms.json");

#[derive(Deserialize)]
struct LibraryFile {
    version: u32,
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

fn load_library() -> Vec<CaseDef> {
    let file: LibraryFile = serde_json::from_str(LIBRARY_JSON).expect("valid JSON");
    assert_eq!(file.version, 1);
    file.cases
        .into_iter()
        .map(|c| CaseDef {
            set: match c.set.as_str() {
                "PLL" => CaseSet::Pll,
                "OLL" => CaseSet::Oll,
                "F2L" => CaseSet::F2l,
                "LBL" => CaseSet::Lbl,
                other => panic!("case '{}': unknown set '{other}'", c.id),
            },
            recognition: match c.recognition.as_str() {
                "oll" => RecogKind::Oll,
                "pll" => RecogKind::Pll,
                "f2l" => RecogKind::F2l,
                "none" => RecogKind::None,
                other => panic!("case '{}': unknown recognition '{other}'", c.id),
            },
            alg: Alg::parse(&c.moves)
                .unwrap_or_else(|e| panic!("case '{}': bad moves: {e}", c.id)),
            id: c.id,
            name: c.name,
            group: c.group,
        })
        .collect()
}

#[test]
fn library_parses_and_builds_recognizer() {
    let lib = load_library();
    // Unique ids.
    let mut ids: Vec<&str> = lib.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(before, ids.len(), "duplicate case ids");
    // Recognizer validates every alg's stage and detects collisions.
    Recognizer::new(lib).unwrap();
}

#[test]
fn every_recognizable_case_roundtrips_from_all_aufs_and_frames() {
    let rec = Recognizer::new(load_library()).unwrap();
    let mut rng = SplitMix64::new(0xA1B2C3);
    for case_idx in 0..rec.defs().len() as u16 {
        let def = rec.case(case_idx);
        if matches!(def.recognition, RecogKind::None) {
            continue;
        }
        let id = def.id.clone();
        let kind = def.recognition;
        for round in 0..12 {
            let s = rec.setup_state(case_idx, &mut rng);
            let m = rec
                .recognize_case(&s, case_idx)
                .unwrap_or_else(|| panic!("{id} round {round}: not recognized"));
            let done = s.applied_alg(&rec.execution_alg(m));
            match kind {
                RecogKind::Oll => assert!(
                    done.is_f2l_solved() && done.is_oll_done(),
                    "{id} round {round}: execution did not finish OLL"
                ),
                RecogKind::Pll => assert!(
                    (0..4).any(|k| done.rotate_u(k).is_solved()),
                    "{id} round {round}: execution did not solve the cube"
                ),
                RecogKind::F2l => assert!(
                    done.is_f2l_solved(),
                    "{id} round {round}: execution did not finish F2L"
                ),
                RecogKind::None => unreachable!(),
            }
        }
    }
}

/// Playback-only cases still need parseable algs that return to *some*
/// consistent state; at minimum the alg must not be empty.
#[test]
fn playback_only_cases_have_algs() {
    for def in Recognizer::new(load_library()).unwrap().defs() {
        assert!(!def.alg.is_empty(), "{}: empty alg", def.id);
    }
}

/// Full-set counts — enforced once M5 data entry is complete.
/// Run with: cargo test -p cube-core -- --ignored
#[test]
#[ignore = "enable when the full library is entered (M5)"]
fn full_library_counts() {
    let lib = load_library();
    let count = |set: CaseSet| lib.iter().filter(|c| c.set == set).count();
    assert_eq!(count(CaseSet::Pll), 21, "PLL must have all 21 cases");
    assert_eq!(count(CaseSet::Oll), 57, "OLL must have all 57 cases");
    assert_eq!(count(CaseSet::F2l), 41, "F2L must have all 41 cases");
    assert!(count(CaseSet::Lbl) >= 8, "LBL needs a teachable set");
    // Every OLL/PLL/F2L case must be recognizable, not playback-only.
    for c in &lib {
        if matches!(c.set, CaseSet::Pll | CaseSet::Oll | CaseSet::F2l) {
            assert!(
                !matches!(c.recognition, RecogKind::None),
                "{}: main-set case must be recognizable",
                c.id
            );
        }
    }
    let _ = FaceletCube::SOLVED;
}
