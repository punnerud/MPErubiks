use cube_core::{Alg, FaceletCube};

#[test]
fn probe_daisy_state() {
    let s = FaceletCube::SOLVED.applied_alg(&Alg::parse("x2 F2 R2 B2 L2").unwrap());
    let face: Vec<char> = (0..9).map(|i| s.0[i].letter()).collect();
    println!("U rows: {:?} {:?} {:?}", &face[0..3], &face[3..6], &face[6..9]);
    // Daisy: yellow (D) center, white (U) edges, yellow (D) corners.
    assert_eq!(face[4], 'D', "center must be yellow");
    for i in [1usize, 3, 5, 7] {
        assert_eq!(face[i], 'U', "edge {i} must be white");
    }
}
