//! Field-data eval: re-runs the matcher on REAL captured cells (uploaded
//! from the phone) against hand-labeled ground truth, and compares the
//! current palette with the classic-only one. Skips silently unless
//! RUBIKS_SCANS_DIR points at the dataset (personal photos live outside
//! the repo).
//!
//! Run: RUBIKS_SCANS_DIR=... cargo test -p cube-vision --test eval_scans -- --nocapture

use cube_vision::{vote_cell, Calibration, Oklab};

/// (capture dir, truth per display cell) — classes: 0 w 1 y 2 r 3 o 4 g 5 b.
/// Labeled from the paired full-frame uploads.
const LABELED: &[(&str, [u8; 9])] = &[
    ("20260731-231959-249", [5, 1, 4, 1, 1, 0, 1, 4, 5]),
    ("20260731-232003-974", [2, 3, 0, 5, 3, 5, 0, 2, 1]),
    ("20260731-232013-499", [0, 4, 0, 0, 5, 0, 4, 1, 1]),
];

fn classic_only() -> Calibration {
    let s = Oklab::from_srgb8;
    Calibration {
        shades: vec![
            (0, s(245, 245, 245)),
            (1, s(255, 213, 0)),
            (2, s(196, 30, 58)),
            (3, s(255, 88, 0)),
            (4, s(0, 158, 96)),
            (5, s(0, 81, 186)),
        ],
    }
}

/// Decoder for the upload server's PNGs (RGBA8, filter 0 rows).
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

fn accuracy(cal: &Calibration, dir: &std::path::Path) -> (usize, usize, Vec<String>) {
    let (mut hit, mut total) = (0, 0);
    let mut misses = Vec::new();
    for (capture, truth) in LABELED {
        let cap_dir = dir.join(capture);
        for (i, &want) in truth.iter().enumerate() {
            let Some((w, h, rgba)) = read_png(&cap_dir.join(format!("cell_{i}.png"))) else {
                continue;
            };
            total += 1;
            let got = vote_cell(&rgba, w, h, cal).color;
            if got == Some(want) {
                hit += 1;
            } else {
                misses.push(format!("{capture} cell{i}: want {want} got {got:?}"));
            }
        }
    }
    (hit, total, misses)
}

#[test]
fn field_captures_match_ground_truth() {
    let Some(dir) = std::env::var_os("RUBIKS_SCANS_DIR") else {
        eprintln!("RUBIKS_SCANS_DIR not set - skipping field eval");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let (old_hit, old_total, _) = accuracy(&classic_only(), &dir);
    let (new_hit, new_total, misses) = accuracy(&Calibration::default_stickers(), &dir);
    assert!(new_total > 0, "no labeled captures found in {dir:?}");
    eprintln!("classic palette: {old_hit}/{old_total}");
    eprintln!("current palette: {new_hit}/{new_total}");
    for m in &misses {
        eprintln!("  MISS {m}");
    }
    assert!(
        new_hit * 10 >= new_total * 8,
        "current palette must reach 80% on the labeled field set ({new_hit}/{new_total})"
    );
    assert!(new_hit >= old_hit, "the new palette must not regress");
}
