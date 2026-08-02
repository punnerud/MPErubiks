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
    ("20260731-233323-283", [2, 4, 2, 2, 5, 3, 4, 1, 0]),
    // Full six-side night run (warm indoor light) — labels hand-read from
    // montages; the six grids sum to EXACTLY nine stickers per class,
    // so the labeling is self-consistent.
    ("20260731-235221-529", [3, 1, 2, 0, 2, 1, 5, 0, 0]),
    ("20260731-235227-226", [4, 1, 1, 2, 4, 5, 2, 2, 0]),
    ("20260731-235231-470", [4, 3, 4, 1, 3, 3, 5, 2, 2]),
    ("20260731-235238-905", [2, 4, 1, 5, 0, 3, 4, 4, 3]),
    ("20260731-235246-015", [5, 4, 0, 5, 5, 0, 1, 2, 5]),
    ("20260731-235252-580", [3, 0, 3, 5, 1, 3, 0, 4, 1]),
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
fn grid_offset_on_a_real_frame() {
    let Some(dir) = std::env::var_os("RUBIKS_SCANS_DIR") else {
        eprintln!("RUBIKS_SCANS_DIR not set - skipping");
        return;
    };
    let path = std::path::PathBuf::from(dir).join("20260731-234151-233/cell_0.png");
    let Some((w, h, rgba)) = read_png(&path) else {
        eprintln!("frame upload not present - skipping");
        return;
    };
    // The guide square: 0.6 x min side, centered (matches the app).
    let side = (0.6 * w.min(h) as f32) as usize;
    let (left, top) = ((w - side) / 2, (h - side) / 2);
    let crop = |ox: i32, oy: i32| -> Vec<u8> {
        let mut out = Vec::with_capacity(side * side * 4);
        for y in 0..side {
            for x in 0..side {
                let sx = (left as i32 + ox + x as i32).clamp(0, w as i32 - 1) as usize;
                let sy = (top as i32 + oy + y as i32).clamp(0, h as i32 - 1) as usize;
                out.extend_from_slice(&rgba[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4]);
            }
        }
        out
    };
    let (dx, dy, conf) = cube_vision::grid_offset(&crop(0, 0), side, side);
    eprintln!("aligned frame: offset ({dx:.1},{dy:.1}) conf {conf:.2}");
    assert!(conf > 0.4, "real frame should show grid structure ({conf})");
    let base = (dx, dy);
    // Crop shifted 30px right: the grid should appear 30px LEFT of it.
    let (dx2, dy2, conf2) = cube_vision::grid_offset(&crop(30, 0), side, side);
    eprintln!("shifted crop: offset ({dx2:.1},{dy2:.1}) conf {conf2:.2}");
    assert!(
        (dx2 - (base.0 - 30.0)).abs() < 9.0 && (dy2 - base.1).abs() < 9.0,
        "shift must be recovered: base {base:?} shifted ({dx2},{dy2})"
    );
}

/// The real uploaded frame contains a cube; an all-background crop of
/// the same photo (the table beside it) must NOT.
#[test]
fn cube_detection_on_a_real_frame() {
    let Some(dir) = std::env::var_os("RUBIKS_SCANS_DIR") else {
        return;
    };
    let path = std::path::PathBuf::from(dir).join("20260731-234151-233/cell_0.png");
    let Some((w, h, rgba)) = read_png(&path) else {
        eprintln!("frame upload not present - skipping");
        return;
    };
    let crop = |cx: usize, cy: usize, side: usize| -> Vec<u8> {
        let mut out = Vec::with_capacity(side * side * 4);
        for y in 0..side {
            for x in 0..side {
                let sx = (cx + x).min(w - 1);
                let sy = (cy + y).min(h - 1);
                out.extend_from_slice(&rgba[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4]);
            }
        }
        out
    };
    let side = (0.6 * w.min(h) as f32) as usize;
    let cube = crop((w - side) / 2, (h - side) / 2, side);
    let fit = cube_vision::grid_fit(&cube, side, side);
    eprintln!("cube crop: is_cube={} conf={:.2}", fit.is_cube, fit.conf);
    assert!(fit.is_cube, "the guide square holds a cube here");

    // Top strip of the same photo: table only.
    let bg_side = side.min(h / 6);
    let bg = crop(0, 0, bg_side);
    let bg_fit = cube_vision::grid_fit(&bg, bg_side, bg_side);
    eprintln!("background crop: is_cube={} conf={:.2}", bg_fit.is_cube, bg_fit.conf);
    assert!(!bg_fit.is_cube, "plain background must not read as a cube");
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
