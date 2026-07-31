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

/// Six reference colors. Class indices are opaque to this crate: with
/// `default_stickers` they follow [white, yellow, red, orange, green, blue];
/// with `from_centers` they follow whatever order the centers were captured
/// in — the app assigns meaning.
#[derive(Clone, Copy, Debug)]
pub struct Calibration {
    pub refs: [Oklab; 6],
}

impl Calibration {
    /// Typical stickered-cube colors under neutral light; good enough for
    /// the first pass until all six centers are captured.
    pub fn default_stickers() -> Calibration {
        Calibration {
            refs: [
                Oklab::from_srgb8(245, 245, 245), // white
                Oklab::from_srgb8(255, 213, 0),   // yellow
                Oklab::from_srgb8(196, 30, 58),   // red
                Oklab::from_srgb8(255, 88, 0),    // orange
                Oklab::from_srgb8(0, 158, 96),    // green
                Oklab::from_srgb8(0, 81, 186),    // blue
            ],
        }
    }

    /// Recalibrate from the six captured center stickers: absorbs the
    /// scene's actual white balance and exposure.
    pub fn from_centers(centers: [Oklab; 6]) -> Calibration {
        Calibration { refs: centers }
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
    let any_allowed = cal.refs.iter().any(|&r| allowed(r));
    let mut best = (f32::INFINITY, 0u8);
    let mut second = f32::INFINITY;
    for (i, &r) in cal.refs.iter().enumerate() {
        if any_allowed && !allowed(r) {
            continue;
        }
        let d = sticker_distance(patch, r);
        if d < best.0 {
            second = best.0;
            best = (d, i as u8);
        } else if d < second {
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
