//! "Min vei" macro plans must actually solve: cross + recognized
//! algorithm segments (+ honest kewb tail when coverage has holes).

use cube_core::{FaceletCube, Move, SplitMix64};

#[test]
fn my_way_plans_solve_random_scrambles() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/table.bin"
    ))
    .expect("run gen-table");
    cube_solver::install_table(&bytes).unwrap();
    let lib = cube_app::library::Library::load();
    let mut rng = SplitMix64::new(21);
    let mut named = 0usize;
    let mut raw = 0usize;
    for round in 0..10 {
        let mut s = FaceletCube::SOLVED;
        for _ in 0..20 {
            s.apply(Move::from_index(rng.below(18) as usize));
        }
        let plan = cube_solver::my_way(&s, &lib.rec, &|_| None)
            .unwrap_or_else(|| panic!("round {round}: no plan"));
        let mut cube = s;
        for seg in &plan.segments {
            cube = cube.applied_alg(&seg.alg);
            if seg.case_idx.is_some() {
                named += 1;
            } else {
                raw += 1;
            }
        }
        assert!(
            (0..6).all(|f| {
                let c = cube.0[f * 9 + 4];
                (0..9).all(|o| cube.0[f * 9 + o] == c)
            }),
            "round {round}: plan must solve the cube"
        );
        assert!(plan.total_ms > 0);
    }
    eprintln!("segments across 10 plans: {named} named, {raw} raw");
    assert!(named > 0, "macro plans should use named algorithms");
}
