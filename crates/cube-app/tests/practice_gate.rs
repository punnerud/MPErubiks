//! Practice-stop gate: from a T-perm state, the guide must stop at the
//! algorithm's own moves (AUF ungated), fast-forward correctly on
//! "I did it", and step normally after "Show me".

use cube_app::screens::solve::GuideState;

#[test]
fn gate_blocks_fast_forwards_and_reveals() {
    let lib = cube_app::library::Library::load();
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/table.bin"
    ))
    .expect("run gen-table");
    cube_solver::install_table(&bytes).unwrap();

    let t_idx = lib.rec.find_by_id("pll-t").unwrap();
    let mut rng = cube_core::SplitMix64::new(11);
    let state = lib.rec.setup_state(t_idx, &mut rng);
    let out = cube_solver::solve_with_hints(&state, &[t_idx], &lib.rec, 6).unwrap();
    assert!(out.guided.is_some(), "T-perm state must guide");
    let mut guide = GuideState::from_output(out, &lib.rec, state, true);

    // Step (simulated) through ungated moves; the gate must appear
    // exactly when the cursor reaches the algorithm's own moves.
    let mut cube = state;
    while guide.gate().is_none() && guide.cursor < guide.solution.0.len() {
        assert!(
            guide.seg_id[guide.cursor].is_none(),
            "ungated stepping must only cross non-segment moves (AUF/filler)"
        );
        cube.apply(guide.solution.0[guide.cursor]);
        guide.cursor += 1;
    }
    let k = guide.gate().expect("the T-perm segment must gate");

    // Fast-forward ("I did it"): apply the segment, cursor to its end.
    let end = guide.seg_end(k);
    assert!(end > guide.cursor, "segment must be non-empty");
    for &m in &guide.solution.0[guide.cursor..end] {
        cube.apply(m);
    }
    guide.cursor = end;
    assert!(guide.gate().is_none(), "past the segment: no gate");

    // Finish any tail and confirm the cube actually solves.
    while guide.cursor < guide.solution.0.len() {
        cube.apply(guide.solution.0[guide.cursor]);
        guide.cursor += 1;
    }
    assert!(cube.is_solved(), "guided path must solve the cube");

    // Reveal path: a fresh guide, revealed segment never gates.
    let out2 = cube_solver::solve_with_hints(&state, &[t_idx], &lib.rec, 6).unwrap();
    let mut g2 = GuideState::from_output(out2, &lib.rec, state, true);
    while g2.gate().is_none() && g2.cursor < g2.solution.0.len() {
        g2.cursor += 1;
    }
    let k2 = g2.gate().unwrap();
    g2.revealed.insert(k2);
    assert!(g2.gate().is_none(), "revealed segment must not gate");
}
