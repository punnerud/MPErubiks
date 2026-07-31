//! Replays a REAL full six-side scan (uploaded cell PNGs) through the whole
//! resolve pipeline offline: histograms -> optimal center assignment ->
//! FACE-space shares -> constraint resolver. Compares grid-table variants,
//! so a physical-mapping bug shows up as "resolves cleanly under table X,
//! fails under table Y". Skips unless RUBIKS_SCANS_DIR is set (the photos
//! are personal and live outside the repo).
//!
//! Run: RUBIKS_SCANS_DIR=... cargo test -p cube-solver --test field_resolve -- --nocapture

use cube_core::Face;
use cube_solver::{assign_classes, resolve_scan, Shares};
use cube_vision::{vote_histogram, Calibration};

/// Complete field runs (capture order F R B D L U each).
const RUNS: &[[&str; 6]] = &[
    [
        "20260731-235221-529",
        "20260731-235227-226",
        "20260731-235231-470",
        "20260731-235238-905",
        "20260731-235246-015",
        "20260731-235252-580",
    ],
    [
        "20260801-000528-970",
        "20260801-000533-500",
        "20260801-000538-295",
        "20260801-000543-057",
        "20260801-000546-761",
        "20260801-000550-807",
    ],
];

const ORDER: [Face; 6] = [Face::F, Face::R, Face::B, Face::D, Face::L, Face::U];

/// MUST mirror CAPTURE_GRID_TO_FACELET in cube-app/src/screens/scan.rs.
const TABLES: [[u8; 9]; 6] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    [8, 7, 6, 5, 4, 3, 2, 1, 0],
    [6, 3, 0, 7, 4, 1, 8, 5, 2],
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
];

/// Decoder for the upload server's PNGs (RGBA8, filter-0 rows).
fn read_png(path: &std::path::Path) -> Option<(usize, usize, Vec<u8>)> {
    let data = std::fs::read(path).ok()?;
    let mut pos = 8;
    let (mut w, mut h) = (0usize, 0usize);
    let mut idat = Vec::new();
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
        let tag = &data[pos + 4..pos + 8];
        match tag {
            b"IHDR" => {
                w = u32::from_be_bytes(data[pos + 8..pos + 12].try_into().ok()?) as usize;
                h = u32::from_be_bytes(data[pos + 12..pos + 16].try_into().ok()?) as usize;
            }
            b"IDAT" => idat.extend_from_slice(&data[pos + 8..pos + 8 + len]),
            _ => {}
        }
        pos += 12 + len;
    }
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).ok()?;
    let stride = w * 4 + 1;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &raw[y * stride..(y + 1) * stride];
        assert_eq!(row[0], 0, "unfiltered rows expected");
        rgba.extend_from_slice(&row[1..]);
    }
    Some((w, h, rgba))
}

fn build_shares(dir: &std::path::Path, run: &[&str; 6]) -> Option<Shares> {
    let cal = Calibration::default_stickers();
    let mut hists = [[[0.0f32; 6]; 9]; 6];
    for (k, capture) in run.iter().enumerate() {
        for g in 0..9 {
            let (w, h, rgba) = read_png(&dir.join(capture).join(format!("cell_{g}.png")))?;
            hists[k][g] = vote_histogram(&rgba, w, h, &cal);
        }
    }
    let centers: [[f32; 6]; 6] = core::array::from_fn(|k| hists[k][4]);
    let capture_class = assign_classes(&centers);
    eprintln!("assignment (capture k -> class): {capture_class:?}");
    const CN: [&str; 6] = ["wht", "yel", "red", "org", "grn", "blu"];
    for k in 0..6 {
        let g: Vec<String> = (0..9)
            .map(|i| {
                let h = hists[k][i];
                let top = (0..6).max_by(|&a, &b| h[a].partial_cmp(&h[b]).unwrap()).unwrap();
                if h[top] < 0.05 { "---".into() } else { format!("{}", CN[top]) }
            })
            .collect();
        eprintln!("capture {k} ({:?}): center hist {:?}", ORDER[k], centers[k].map(|v| (v * 100.0) as i32));
        eprintln!("   argmax grid: {g:?}");
    }
    let mut class_to_face = [Face::U; 6];
    for (k, &face) in ORDER.iter().enumerate() {
        class_to_face[capture_class[k]] = face;
    }
    let mut shares: Shares = [[0.0; 6]; 54];
    for (k, &face) in ORDER.iter().enumerate() {
        for (g, &off) in TABLES[k].iter().enumerate() {
            let facelet = face as usize * 9 + off as usize;
            for class in 0..6 {
                shares[facelet][class_to_face[class] as usize] += hists[k][g][class];
            }
        }
    }
    Some(shares)
}

#[test]
fn latest_field_run_resolves() {
    let Some(dir) = std::env::var_os("RUBIKS_SCANS_DIR") else {
        eprintln!("RUBIKS_SCANS_DIR not set - skipping field replay");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    for run in RUNS {
        if !dir.join(run[0]).exists() {
            eprintln!("run {} not present - skipping", run[0]);
            continue;
        }
        let shares = build_shares(&dir, run).expect("readable capture PNGs");
        // Diff argmax vs the hand-labeled truth (first run only).
        if run[0] == "20260731-235221-529" {
            const LABELS: [[u8; 9]; 6] = [
                [3, 1, 2, 0, 2, 1, 5, 0, 0],
                [4, 1, 1, 2, 4, 5, 2, 2, 0],
                [4, 3, 4, 1, 3, 3, 5, 2, 2],
                [2, 4, 1, 5, 0, 3, 4, 4, 3],
                [5, 4, 0, 5, 5, 0, 1, 2, 5],
                [3, 0, 3, 5, 1, 3, 0, 4, 1],
            ];
            let class_to_face = [Face::D, Face::U, Face::F, Face::B, Face::R, Face::L];
            for (k, &face) in ORDER.iter().enumerate() {
                for (g, &off) in TABLES[k].iter().enumerate() {
                    let i = face as usize * 9 + off as usize;
                    let truth = class_to_face[LABELS[k][g] as usize];
                    let am = (0..6)
                        .max_by(|&a, &b| shares[i][a].partial_cmp(&shares[i][b]).unwrap())
                        .unwrap();
                    if am != truth as usize {
                        eprintln!(
                            "  facelet {i} (cap{k} cell{g}): truth {truth:?} argmax {am} shares {:?}",
                            shares[i].map(|v| (v * 100.0) as i32)
                        );
                    }
                }
            }
        }
        let outcome = resolve_scan(&shares);
        // How far did the resolver move from the raw evidence?
        let corrections = outcome.as_ref().ok().map(|cube| {
            (0..54)
                .filter(|&i| {
                    let argmax = (0..6).max_by(|&a, &b| {
                        shares[i][a].partial_cmp(&shares[i][b]).unwrap()
                    });
                    Some(cube.0[i] as usize) != argmax
                })
                .count()
        });
        eprintln!(
            "run {}: {} (corrections vs argmax: {corrections:?})",
            run[0],
            match &outcome {
                Ok(_) => "RESOLVED to a legal cube".to_string(),
                Err(e) => format!("failed: {e}"),
            }
        );
        assert!(
            outcome.is_ok(),
            "field run {} must constraint-resolve to a legal cube",
            run[0]
        );
    }
}
