//! Property and identity tests that lock down the geometrically derived
//! move tables, the notation machinery and the case recognizer.

use cube_core::{
    Alg, CaseDef, CaseSet, Face, FaceletCube, Move, RecogKind, Recognizer, RotAxis,
    SplitMix64, Turns,
};

const SOLVED: FaceletCube = FaceletCube::SOLVED;

fn alg(s: &str) -> Alg {
    Alg::parse(s).unwrap_or_else(|e| panic!("parse '{s}': {e}"))
}

fn applied(s: &str) -> FaceletCube {
    SOLVED.applied_alg(&alg(s))
}

// ---------------------------------------------------------------- moves

#[test]
fn every_quarter_move_is_a_permutation_of_order_four() {
    for base in [
        "U", "R", "F", "D", "L", "B", "Uw", "Rw", "Fw", "Dw", "Lw", "Bw", "M", "E", "S", "x", "y",
        "z",
    ] {
        let a = alg(base);
        let mut s = SOLVED;
        for k in 1..=4 {
            s.apply_alg(&a);
            let back_to_start = s == SOLVED;
            assert_eq!(back_to_start, k == 4, "{base}^{k}");
        }
    }
}

#[test]
fn move_times_inverse_is_identity() {
    for i in 0..54 {
        let m = Move::from_index(i);
        let s = SOLVED.applied(m).applied(m.inverse());
        assert_eq!(s, SOLVED, "move {m} (index {i})");
        // Notation round-trips through the parser too.
        assert_eq!(alg(&m.to_string()).0, vec![m], "display/parse {m}");
    }
}

#[test]
fn half_equals_quarter_twice() {
    for f in ["U", "R", "F", "D", "L", "B", "M", "E", "S", "x", "y", "z", "Rw"] {
        assert_eq!(applied(&format!("{f} {f}")), applied(&format!("{f}2")), "{f}");
    }
}

#[test]
fn sexy_move_six_times_is_identity() {
    let mut s = SOLVED;
    for _ in 0..6 {
        s.apply_alg(&alg("R U R' U'"));
    }
    assert_eq!(s, SOLVED);
}

#[test]
fn slice_identities() {
    // M = L' x' R  (as state transformations)
    assert_eq!(applied("M"), applied("x' L' R"), "M = x' L' R");
    // E = D' y' U ... equivalently E = y' D' U? E follows D: E = y' U D'?
    // Lock the convention geometrically instead: E = whole-cube y' with U
    // and D layers restored.
    assert_eq!(applied("E"), applied("y' U D'"), "E = y' U D'");
    // S follows F: S = z with F and B restored.
    assert_eq!(applied("S"), applied("z F' B"), "S = z F' B");
}

#[test]
fn rotation_conventions() {
    // y sends the F-face content to L (a U turn of the whole cube).
    let s = SOLVED.applied(Move::Rot(RotAxis::Y, Turns::Cw));
    assert_eq!(s.center(Face::L), Face::F, "y: F content shows at L");
    assert_eq!(s.center(Face::F), Face::R, "y: R content shows at F");
    // x sends F to U (an R turn of the whole cube).
    let s = SOLVED.applied(Move::Rot(RotAxis::X, Turns::Cw));
    assert_eq!(s.center(Face::U), Face::F, "x: F content shows at U");
    // z sends U to R (an F turn of the whole cube).
    let s = SOLVED.applied(Move::Rot(RotAxis::Z, Turns::Cw));
    assert_eq!(s.center(Face::R), Face::U, "z: U content shows at R");
}

#[test]
fn wide_move_equals_face_plus_rotation() {
    // Uw = y ∘ D (rotate whole cube, restore bottom layer).
    assert_eq!(applied("Uw"), applied("D y"), "Uw");
    assert_eq!(applied("Rw"), applied("L x"), "Rw");
    assert_eq!(applied("Fw"), applied("B z"), "Fw");
}

fn random_alg(rng: &mut SplitMix64, len: usize, full_zoo: bool) -> Alg {
    let mut moves = Vec::with_capacity(len);
    for _ in 0..len {
        let idx = if full_zoo {
            rng.below(54) as usize
        } else {
            rng.below(18) as usize // face moves only
        };
        moves.push(Move::from_index(idx));
    }
    Alg::new(moves)
}

#[test]
fn alg_times_inverse_is_identity_1000() {
    let mut rng = SplitMix64::new(0xC0FFEE);
    for round in 0..1000 {
        let len = (5 + rng.below(20)) as usize;
        let a = random_alg(&mut rng, len, true);
        let s = SOLVED.applied_alg(&a).applied_alg(&a.inverse());
        assert_eq!(s, SOLVED, "round {round}: {a}");
    }
}

#[test]
fn parser_roundtrip() {
    for src in [
        "R U R' U'",
        "R U2 R' U' R U' R'",
        "M2 U M U2 M' U M2",
        "Rw U Rw' F2 x y' z2",
        "M E S M' E' S'",
    ] {
        let a = alg(src);
        let printed = a.to_string();
        assert_eq!(alg(&printed), a, "{src} -> {printed}");
        assert_eq!(printed, src.replace("  ", " "), "{src}");
    }
    // Groups expand and invert.
    assert_eq!(alg("(R U R' U')2"), alg("R U R' U' R U R' U'"));
    assert_eq!(alg("(R U)'"), alg("U' R'"));
    assert_eq!(alg("(R U)3"), alg("R U R U R U"));
    // Lowercase wide aliases.
    assert_eq!(alg("u r'"), alg("Uw Rw'"));
}

#[test]
fn face_moves_only_matches_up_to_rotation_500() {
    let mut rng = SplitMix64::new(0xDECAF);
    for round in 0..500 {
        let len = (3 + rng.below(15)) as usize;
        let a = random_alg(&mut rng, len, true);
        let full = SOLVED.applied_alg(&a).normalize_orientation();
        let reduced = SOLVED.applied_alg(&a.face_moves_only()).normalize_orientation();
        assert_eq!(full, reduced, "round {round}: {a}");
        // And the reduction really is face moves only.
        assert!(a
            .face_moves_only()
            .0
            .iter()
            .all(|m| matches!(m, Move::Face(..))));
    }
}

#[test]
fn in_y_frame_is_conjugation_by_y() {
    let mut rng = SplitMix64::new(0xBEEF);
    for _ in 0..200 {
        let len = (2 + rng.below(10)) as usize;
        let a = random_alg(&mut rng, len, true);
        let scramble = random_alg(&mut rng, 12, false);
        let s = SOLVED.applied_alg(&scramble);
        for j in 0..4u8 {
            // applying a.in_y_frame(j) to s == y^-j( a( y^j(s) ) )
            let lhs = s.applied_alg(&a.in_y_frame(j));
            let rhs = s
                .rotate_y(j)
                .applied_alg(&a)
                .rotate_y((4 - j) % 4);
            assert_eq!(lhs, rhs, "j={j}, a={a}");
        }
    }
}

#[test]
fn facelet_string_roundtrip() {
    let s = applied("R U R' U' M2 x (R U R' U')2");
    let text = s.to_facelet_string();
    assert_eq!(FaceletCube::from_facelet_string(&text).unwrap(), s);
    assert_eq!(
        SOLVED.to_facelet_string(),
        "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB"
    );
}

#[test]
fn normalize_orientation_undoes_any_rotation() {
    let mut rng = SplitMix64::new(7);
    for _ in 0..100 {
        let scramble = random_alg(&mut rng, 10, false);
        let s = SOLVED.applied_alg(&scramble);
        let mut rotated = s;
        for _ in 0..(1 + rng.below(5)) {
            let idx = 45 + rng.below(9) as usize; // a random rotation move
            rotated.apply(Move::from_index(idx));
        }
        assert_eq!(rotated.normalize_orientation(), s);
    }
}

#[test]
fn normalize_survives_illegal_centers() {
    // A bad scan can duplicate centers; normalization must not panic and
    // must leave the state untouched for validation to report.
    let mut s = FaceletCube::SOLVED;
    s.0[4] = Face::R; // U center painted red: no rotation can fix this
    assert_eq!(s.normalize_orientation(), s);
}

#[test]
fn stage_predicates() {
    assert!(SOLVED.is_solved());
    assert!(SOLVED.is_f2l_solved());
    assert!(SOLVED.is_oll_done());
    let s = applied("R U R' U'");
    assert!(!s.is_solved());
    assert!(!s.is_f2l_solved()); // sexy move disturbs the FR pair
    // Sune only affects the last layer.
    let sune = applied("R U R' U R U2 R'");
    assert!(!sune.is_solved());
    assert!(sune.is_f2l_solved());
    assert!(!sune.is_oll_done());
    // T-perm leaves orientation but permutes LL.
    let t = applied("R U R' U' R' F R2 U' R' U' R U R' F'");
    assert!(t.is_f2l_solved());
    assert!(t.is_oll_done());
    assert!(!t.is_solved());
    // A solved-but-rotated cube still counts as solved.
    assert!(applied("x y2 z'").is_solved());
}

// ------------------------------------------------------------ recognizer

fn mini_library() -> Vec<CaseDef> {
    let case = |id: &str, set, name: &str, group: &str, moves: &str, recognition| CaseDef {
        id: id.into(),
        set,
        name: name.into(),
        group: group.into(),
        alg: alg(moves),
        recognition,
    };
    vec![
        case(
            "oll-27",
            CaseSet::Oll,
            "Sune",
            "ocll",
            "R U R' U R U2 R'",
            RecogKind::Oll,
        ),
        case(
            "oll-26",
            CaseSet::Oll,
            "Anti-Sune",
            "ocll",
            "R U2 R' U' R U' R'",
            RecogKind::Oll,
        ),
        case(
            "pll-t",
            CaseSet::Pll,
            "T-Perm",
            "adjacent",
            "R U R' U' R' F R2 U' R' U' R U R' F'",
            RecogKind::Pll,
        ),
        case(
            "pll-ua",
            CaseSet::Pll,
            "Ua-Perm",
            "edges",
            "M2 U M U2 M' U M2",
            RecogKind::Pll,
        ),
        case(
            "pll-h",
            CaseSet::Pll,
            "H-Perm",
            "edges",
            "M2 U M2 U2 M2 U M2",
            RecogKind::Pll,
        ),
        case(
            "f2l-1",
            CaseSet::F2l,
            "Basic pair",
            "basic",
            "U R U' R'",
            RecogKind::F2l,
        ),
        case(
            "f2l-2",
            CaseSet::F2l,
            "Basic pair front",
            "basic",
            "U' F' U F",
            RecogKind::F2l,
        ),
        case(
            "lbl-sune",
            CaseSet::Lbl,
            "Sune (beginner)",
            "top",
            "R U R' U R U2 R'",
            RecogKind::None,
        ),
    ]
}

#[test]
fn recognizer_builds_from_mini_library() {
    Recognizer::new(mini_library()).unwrap();
}

#[test]
fn setup_state_is_recognized_and_execution_solves_it() {
    let rec = Recognizer::new(mini_library()).unwrap();
    let mut rng = SplitMix64::new(42);
    for case_idx in 0..rec.defs().len() as u16 {
        if matches!(rec.case(case_idx).recognition, RecogKind::None) {
            continue;
        }
        for round in 0..25 {
            let id = &rec.case(case_idx).id;
            let s = rec.setup_state(case_idx, &mut rng);
            let m = rec
                .recognize_case(&s, case_idx)
                .unwrap_or_else(|| panic!("{id} round {round}: setup state not recognized"));
            let done = s.applied_alg(&rec.execution_alg(m));
            // After executing, the stage the case solves must be complete
            // (possibly up to a final U turn for permutation cases).
            match rec.case(case_idx).recognition {
                RecogKind::Oll => {
                    assert!(done.is_f2l_solved() && done.is_oll_done(), "{id} r{round}")
                }
                RecogKind::Pll => {
                    let solved_up_to_auf = (0..4).any(|k| done.rotate_u(k).is_solved());
                    assert!(solved_up_to_auf, "{id} r{round}");
                }
                RecogKind::F2l => {
                    assert!(done.is_f2l_solved(), "{id} r{round}")
                }
                RecogKind::None => unreachable!(),
            }
        }
    }
}

#[test]
fn recognition_is_auf_and_y_invariant() {
    let rec = Recognizer::new(mini_library()).unwrap();
    let sune = rec.find_by_id("oll-27").unwrap();
    let t = rec.find_by_id("pll-t").unwrap();
    for auf in 0..4u8 {
        for y in 0..4u8 {
            // Build: rotate solved by y, U-turn `auf` times, apply inverse
            // Sune in that frame.
            let mut s = SOLVED.rotate_y(y);
            s = s.rotate_u(auf);
            s.apply_alg(&alg("R U R' U R U2 R'").inverse().in_y_frame(0));
            let s = s.normalize_orientation();
            let m = rec.recognize(&s, &[CaseSet::Oll]);
            assert!(
                m.iter().any(|m| m.case_idx == sune),
                "sune auf={auf} y={y}"
            );

            let mut p = SOLVED.rotate_y(y).rotate_u(auf);
            p.apply_alg(&alg("R U R' U' R' F R2 U' R' U' R U R' F'").inverse());
            let p = p.normalize_orientation();
            let m = rec.recognize(&p, &[CaseSet::Pll]);
            assert!(m.iter().any(|m| m.case_idx == t), "t auf={auf} y={y}");
        }
    }
}

#[test]
fn solved_and_wrong_stage_states_do_not_match() {
    let rec = Recognizer::new(mini_library()).unwrap();
    assert!(rec.recognize(&SOLVED, &[CaseSet::Oll, CaseSet::Pll, CaseSet::F2l]).is_empty());
    // A scrambled mid-solve state matches nothing from the mini library.
    let messy = applied("R U F2 L D' B U2 R' F");
    assert!(rec
        .recognize(&messy, &[CaseSet::Oll, CaseSet::Pll])
        .is_empty());
}
