//! Sticker color classification from camera pixels.
//!
//! Pure functions over raw RGBA patches — no I/O, no platform code — so the
//! classifier is testable on the host with synthetic fixtures. Colors are
//! compared in Oklab, where Euclidean-ish distances track perception; the
//! lightness axis is down-weighted because exposure varies far more between
//! frames than chroma does.

/// A color in the Oklab space (L ≈ 0..1, a/b roughly -0.4..0.4).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Oklab {
    pub fn from_srgb8(r: u8, g: u8, b: u8) -> Oklab {
        let lin = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let (r, g, b) = (lin(r), lin(g), lin(b));
        let l = 0.412_221_46 * r + 0.536_332_54 * g + 0.051_445_995 * b;
        let m = 0.211_903_5 * r + 0.680_699_55 * g + 0.107_396_96 * b;
        let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
        let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
        Oklab {
            l: 0.210_454_26 * l + 0.793_617_79 * m - 0.004_072_047 * s,
            a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_71 * s,
            b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        }
    }

    /// Chroma (distance from the gray axis).
    pub fn chroma(self) -> f32 {
        (self.a * self.a + self.b * self.b).sqrt()
    }
}

/// Perceptual distance with lightness down-weighted (exposure robustness).
pub fn sticker_distance(x: Oklab, y: Oklab) -> f32 {
    let dl = (x.l - y.l) * 0.6;
    let da = x.a - y.a;
    let db = x.b - y.b;
    (dl * dl + da * da + db * db).sqrt()
}

/// Reduce an RGBA patch to one robust color: per-channel median (survives
/// specular glare blobs where a mean would not), then convert to Oklab.
pub fn srgb_patch_to_oklab(rgba: &[u8], width: usize, height: usize) -> Oklab {
    assert_eq!(rgba.len(), width * height * 4, "patch buffer size");
    let n = width * height;
    let mut r: Vec<u8> = Vec::with_capacity(n);
    let mut g: Vec<u8> = Vec::with_capacity(n);
    let mut b: Vec<u8> = Vec::with_capacity(n);
    for px in rgba.chunks_exact(4) {
        r.push(px[0]);
        g.push(px[1]);
        b.push(px[2]);
    }
    let median = |v: &mut Vec<u8>| {
        let mid = v.len() / 2;
        *v.select_nth_unstable(mid).1
    };
    Oklab::from_srgb8(median(&mut r), median(&mut g), median(&mut b))
}

/// Reference shades. Each CLASS (0..6: white yellow red orange green
/// blue) may have SEVERAL shades — classic stickers and measured neon
/// ring-cube shades both vote for the same class. Class indices are
/// opaque to this crate; the app assigns meaning.
#[derive(Clone, Debug)]
pub struct Calibration {
    pub shades: Vec<(u8, Oklab)>,
}

impl Calibration {
    /// Classic sticker shades PLUS neon ring-cube shades measured from
    /// real captures (see the scan-eval dataset): greenish neon yellow,
    /// yellowish neon green, salmon red, amber orange, sky blue.
    pub fn default_stickers() -> Calibration {
        let s = Oklab::from_srgb8;
        Calibration {
            shades: vec![
                (0, s(245, 245, 245)), // white
                (1, s(255, 213, 0)),   // yellow classic
                (1, s(193, 204, 26)),  // yellow neon (greenish)
                (2, s(196, 30, 58)),   // red classic
                (2, s(215, 78, 51)),   // red neon (salmon)
                (2, s(224, 125, 99)),  // red neon washed (bright light)
                (3, s(255, 88, 0)),    // orange classic
                (3, s(222, 140, 25)),  // orange neon (amber)
                (4, s(0, 158, 96)),    // green classic
                (4, s(150, 200, 80)),  // green neon (yellowish)
                (5, s(0, 81, 186)),    // blue classic
                (5, s(45, 165, 210)),  // blue neon (sky)
            ],
        }
    }

    /// Recalibrate from six captured center stickers: absorbs the scene's
    /// actual white balance (classes = capture order).
    pub fn from_centers(centers: [Oklab; 6]) -> Calibration {
        Calibration {
            shades: centers
                .iter()
                .enumerate()
                .map(|(i, &c)| (i as u8, c))
                .collect(),
        }
    }
}

/// One classified sticker.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Classified {
    /// Class index into `Calibration::refs`, or `None` when nothing is
    /// close enough (occlusion, motion blur, off-cube background).
    pub color: Option<u8>,
    /// 1 - d_best/d_second, clamped to [0, 1]: how unambiguous the match is.
    pub confidence: f32,
}

/// Absolute distance gate: beyond this, the patch is "not a sticker color".
const MAX_STICKER_DISTANCE: f32 = 0.25;
/// Sticker colors are never this dark; this rejects the black plastic
/// between stickers when the sampling grid is slightly misaligned.
const MIN_STICKER_LIGHTNESS: f32 = 0.30;

/// Whitish/colored split: sticker whites sit near the gray axis, every
/// real cube color far from it. Gating on chroma makes white vs color a
/// STRUCTURAL decision — a warm-lit white can no longer drift into red.
const WHITISH_CHROMA: f32 = 0.08;

pub fn classify_one(patch: Oklab, cal: &Calibration) -> Classified {
    let patch_chroma = patch.chroma();
    let allowed = |r: Oklab| -> bool {
        if patch_chroma < 0.06 && patch.l > 0.5 {
            r.chroma() < WHITISH_CHROMA // near-gray patch: whitish refs only
        } else if patch_chroma > 0.12 {
            r.chroma() >= WHITISH_CHROMA // saturated patch: never white
        } else {
            true
        }
    };
    let any_allowed = cal.shades.iter().any(|&(_, r)| allowed(r));
    let mut best = (f32::INFINITY, 0u8);
    // Runner-up distance from a DIFFERENT class: two shades of the same
    // color must not depress confidence.
    let mut second = f32::INFINITY;
    for &(class, r) in &cal.shades {
        if any_allowed && !allowed(r) {
            continue;
        }
        let d = sticker_distance(patch, r);
        if d < best.0 {
            if class != best.1 {
                second = best.0;
            }
            best = (d, class);
        } else if class != best.1 && d < second {
            second = d;
        }
    }
    let confidence = if !second.is_finite() {
        // Only one structurally allowed candidate: unambiguous by design.
        if best.0 <= MAX_STICKER_DISTANCE {
            1.0
        } else {
            0.0
        }
    } else if second > 0.0 {
        (1.0 - best.0 / second).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let accepted = best.0 <= MAX_STICKER_DISTANCE && patch.l >= MIN_STICKER_LIGHTNESS;
    Classified {
        color: accepted.then_some(best.1),
        confidence,
    }
}

/// Classify the nine sampled patches of one face.
pub fn classify_face(patches: &[Oklab; 9], cal: &Calibration) -> [Classified; 9] {
    core::array::from_fn(|i| classify_one(patches[i], cal))
}

/// Minimum share of a cell's pixels that must agree for a color to win.
/// Low on purpose: with a misaligned grid the sticker may cover only part
/// of its cell — a CONSISTENT recurring color is the signal, not area.
pub const MIN_VOTE_SHARE: f32 = 0.05;

/// Robust cell classification by per-pixel VOTING: every pixel close to
/// one of the six reference colors votes; the winner needs `MIN_VOTE_SHARE`
/// of all pixels and a clear margin over the runner-up. Survives grid
/// misalignment, plastic borders and background inside the cell.
pub fn vote_cell(rgba: &[u8], width: usize, height: usize, cal: &Calibration) -> Classified {
    assert_eq!(rgba.len(), width * height * 4, "cell buffer size");
    let mut votes = [0u32; 6];
    let mut total = 0u32;
    // Subsample every other pixel: plenty of votes, half the work.
    for px in rgba.chunks_exact(4).step_by(2) {
        total += 1;
        let c = Oklab::from_srgb8(px[0], px[1], px[2]);
        // Dead-zone gate: skin/shadow sits at warm LOW chroma (~0.06-0.09)
        // and poisons both white and orange votes. Only decisively gray
        // pixels may vote white; only decisively saturated ones a color.
        let chroma = c.chroma();
        let decisive_white = chroma < 0.04 && c.l > 0.70;
        let decisive_color = chroma > 0.10;
        if !decisive_white && !decisive_color {
            continue;
        }
        if let Some(k) = classify_one(c, cal).color {
            let is_white_class = k == 0
                || cal
                    .shades
                    .iter()
                    .any(|&(cls, r)| cls == k && r.chroma() < 0.08);
            if (decisive_white && is_white_class) || (decisive_color && !is_white_class) {
                votes[k as usize] += 1;
            }
        }
    }
    if total == 0 {
        return Classified {
            color: None,
            confidence: 0.0,
        };
    }
    // COLOR TRUMPS WHITE: overexposure, dotted textures and gray
    // backgrounds all masquerade as white votes, but a real white sticker
    // has essentially zero saturated votes. So decide among the color
    // classes first; white only wins when no color qualifies.
    let whitish = |k: usize| {
        cal.shades
            .iter()
            .any(|&(cls, r)| cls as usize == k && r.chroma() < WHITISH_CHROMA)
    };
    let mut color_order: Vec<usize> = (0..6).filter(|&k| !whitish(k)).collect();
    color_order.sort_by_key(|&i| std::cmp::Reverse(votes[i]));
    if let (Some(&win), runner) = (color_order.first(), color_order.get(1)) {
        let win_votes = votes[win];
        let runner_votes = runner.map(|&r| votes[r]).unwrap_or(0);
        let share = win_votes as f32 / total as f32;
        if share >= MIN_VOTE_SHARE && win_votes >= runner_votes.saturating_mul(7) / 6 + 1 {
            return Classified {
                color: Some(win as u8),
                confidence: (share * 4.0).clamp(0.0, 1.0),
            };
        }
    }
    let white_class = (0..6).filter(|&k| whitish(k)).max_by_key(|&k| votes[k]);
    if let Some(win) = white_class {
        let share = votes[win] as f32 / total as f32;
        if share >= 0.08 {
            return Classified {
                color: Some(win as u8),
                confidence: (share * 3.0).clamp(0.0, 1.0),
            };
        }
    }
    Classified {
        color: None,
        confidence: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny deterministic LCG so fixtures need no dependencies.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) as u32
        }
        fn jitter(&mut self, base: u8, amp: i32) -> u8 {
            let d = (self.next() % (2 * amp as u32 + 1)) as i32 - amp;
            (i32::from(base) + d).clamp(0, 255) as u8
        }
    }

    /// Synthetic 20x20 sticker patch: base color + noise + brightness
    /// gradient + a specular glare blob in one corner.
    fn patch(rgb: [u8; 3], seed: u64, glare: bool) -> Vec<u8> {
        let mut rng = Lcg(seed);
        let (w, h) = (20usize, 20usize);
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let grad = (x as i32 - 10) * 2; // linear lighting gradient
                let mut px = [0u8; 3];
                for c in 0..3 {
                    let base = (i32::from(rgb[c]) + grad).clamp(0, 255) as u8;
                    px[c] = rng.jitter(base, 10);
                }
                if glare && x < 5 && y < 5 {
                    px = [255, 255, 255]; // blown-out specular highlight
                }
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
        }
        out
    }

    const STICKERS: [[u8; 3]; 6] = [
        [245, 245, 245],
        [255, 213, 0],
        [196, 30, 58],
        [255, 88, 0],
        [0, 158, 96],
        [0, 81, 186],
    ];

    #[test]
    fn clean_patches_classify_correctly_with_confidence() {
        let cal = Calibration::default_stickers();
        for (class, rgb) in STICKERS.iter().enumerate() {
            for seed in 0..20 {
                let p = patch(*rgb, seed, false);
                let ok = srgb_patch_to_oklab(&p, 20, 20);
                let c = classify_one(ok, &cal);
                assert_eq!(c.color, Some(class as u8), "class {class} seed {seed}");
                assert!(
                    c.confidence > 0.5,
                    "class {class} seed {seed}: confidence {}",
                    c.confidence
                );
            }
        }
    }

    #[test]
    fn glare_blob_does_not_flip_the_median() {
        let cal = Calibration::default_stickers();
        // Glare covers 25 of 400 pixels; the median must survive it.
        for (class, rgb) in STICKERS.iter().enumerate() {
            let p = patch(*rgb, 99, true);
            let ok = srgb_patch_to_oklab(&p, 20, 20);
            let c = classify_one(ok, &cal);
            assert_eq!(c.color, Some(class as u8), "class {class} with glare");
        }
    }

    #[test]
    fn dark_background_and_plastic_are_rejected() {
        let cal = Calibration::default_stickers();
        // Black cube plastic (misaligned grid) and a dark desk must reject.
        // Note: mid-tone backgrounds (skin, wood) CAN color-match a sticker
        // — that is by design; cross-face validation catches those later.
        for rgb in [[15u8, 15, 15], [40, 35, 30]] {
            let p = patch(rgb, 7, false);
            let ok = srgb_patch_to_oklab(&p, 20, 20);
            let c = classify_one(ok, &cal);
            assert_eq!(c.color, None, "{rgb:?} must not classify as a sticker");
        }
    }

    #[test]
    fn voting_survives_misalignment_and_rejects_background() {
        let cal = Calibration::default_stickers();
        // Cell 40x40 where only the central 22% is green sticker, the rest
        // near-black plastic: voting must still find green.
        let (w, h) = (40usize, 40usize);
        let mut buf = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let sticker = (12..31).contains(&x) && (12..31).contains(&y);
                let px = if sticker { [0u8, 158, 96] } else { [18, 18, 20] };
                buf.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
        }
        let c = vote_cell(&buf, w, h, &cal);
        assert_eq!(c.color, Some(4), "green wins by votes, got {:?}", c.color);
        // All-plastic cell: nothing wins.
        let dark: Vec<u8> = (0..w * h).flat_map(|_| [18u8, 18, 20, 255]).collect();
        assert_eq!(vote_cell(&dark, w, h, &cal).color, None);
    }

    #[test]
    fn warm_tinted_white_stays_white() {
        // Regression: white under a red-ish cast must NEVER classify as
        // red — chroma gating keeps near-gray patches whitish.
        let cal = Calibration::default_stickers();
        for rgb in [[250u8, 238, 230], [235, 225, 228], [255, 245, 235]] {
            let p = patch(rgb, 5, false);
            let ok = srgb_patch_to_oklab(&p, 20, 20);
            let c = classify_one(ok, &cal);
            assert_eq!(c.color, Some(0), "{rgb:?} must stay white, got {:?}", c.color);
        }
    }

    #[test]
    fn warm_light_white_vs_yellow_needs_recalibration() {
        // Under warm light, white shifts toward yellow. With center-based
        // recalibration the ambiguity resolves.
        let warm = |rgb: [u8; 3]| -> [u8; 3] {
            [
                rgb[0].saturating_add(8),
                rgb[1],
                (f32::from(rgb[2]) * 0.62) as u8,
            ]
        };
        let warmed: Vec<[u8; 3]> = STICKERS.iter().map(|&c| warm(c)).collect();
        let recal = Calibration::from_centers(core::array::from_fn(|i| {
            let p = patch(warmed[i], 1000 + i as u64, false);
            srgb_patch_to_oklab(&p, 20, 20)
        }));
        for (class, rgb) in warmed.iter().enumerate() {
            for seed in 0..10 {
                let p = patch(*rgb, 2000 + seed, false);
                let ok = srgb_patch_to_oklab(&p, 20, 20);
                let c = classify_one(ok, &recal);
                assert_eq!(c.color, Some(class as u8), "warm class {class} seed {seed}");
                assert!(c.confidence > 0.3, "warm class {class}: {}", c.confidence);
            }
        }
    }
}
