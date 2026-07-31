//! Tap-to-select: cast a ray from the camera through the tapped pixel and
//! find which face layer the hit sticker belongs to. Pure math, testable.

use crate::camera::OrbitCamera;
use crate::scene::CUBIE_SPACING;
use cube_core::Face;

/// Pick the face layer under a tap. `ndc` is the tap position in the view
/// rect mapped to (-1..1, -1..1) with +y up. Returns the OUTER face whose
/// sticker was hit (tapping any right-side sticker selects R, etc.).
pub fn pick_face(orbit: &OrbitCamera, aspect: f32, ndc: (f32, f32)) -> Option<Face> {
    let (origin, dir) = orbit.ray(aspect, ndc);

    let mut best: Option<(f32, Face)> = None;
    for gx in -1i8..=1 {
        for gy in -1i8..=1 {
            for gz in -1i8..=1 {
                let center = [
                    f32::from(gx) * CUBIE_SPACING,
                    f32::from(gy) * CUBIE_SPACING,
                    f32::from(gz) * CUBIE_SPACING,
                ];
                let Some((t, axis, sign)) = ray_aabb(origin, dir, center, 0.5) else {
                    continue;
                };
                // Only a sticker face counts: the cubie must sit on the
                // outer shell in the hit-normal direction.
                let grid = [gx, gy, gz];
                if grid[axis] != sign {
                    continue;
                }
                let face = match (axis, sign) {
                    (0, 1) => Face::R,
                    (0, -1) => Face::L,
                    (1, 1) => Face::U,
                    (1, -1) => Face::D,
                    (2, 1) => Face::F,
                    _ => Face::B,
                };
                if best.map_or(true, |(bt, _)| t < bt) {
                    best = Some((t, face));
                }
            }
        }
    }
    best.map(|(_, f)| f)
}

/// Slab-method ray/AABB intersection. Returns (t_enter, entry axis, entry
/// side sign) for the nearest forward hit.
fn ray_aabb(
    origin: [f32; 3],
    dir: [f32; 3],
    center: [f32; 3],
    half: f32,
) -> Option<(f32, usize, i8)> {
    let mut t_enter = f32::NEG_INFINITY;
    let mut t_exit = f32::INFINITY;
    let mut enter_axis = 0usize;
    let mut enter_sign = 0i8;
    for axis in 0..3 {
        let lo = center[axis] - half;
        let hi = center[axis] + half;
        if dir[axis].abs() < 1e-6 {
            if origin[axis] < lo || origin[axis] > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir[axis];
        let (t0, t1, sign) = if inv >= 0.0 {
            ((lo - origin[axis]) * inv, (hi - origin[axis]) * inv, -1i8)
        } else {
            ((hi - origin[axis]) * inv, (lo - origin[axis]) * inv, 1i8)
        };
        if t0 > t_enter {
            t_enter = t0;
            enter_axis = axis;
            enter_sign = sign;
        }
        t_exit = t_exit.min(t1);
        if t_enter > t_exit {
            return None;
        }
    }
    (t_enter > 0.0).then_some((t_enter, enter_axis, enter_sign))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_tap_hits_a_face_and_edges_marked_outward() {
        let orbit = OrbitCamera::default();
        // Dead center should hit SOMETHING (the cube fills the middle).
        let face = pick_face(&orbit, 1.3, (0.0, 0.0));
        assert!(face.is_some(), "center tap must hit the cube");
        // Far corner taps miss.
        assert_eq!(pick_face(&orbit, 1.3, (0.95, 0.95)), None);
    }

    #[test]
    fn default_camera_sees_exactly_u_f_r() {
        // The default orbit looks from up-front-right: scanning the view,
        // the visible sticker faces must be exactly {U, F, R} — never the
        // far sides.
        let orbit = OrbitCamera::default();
        let mut seen = std::collections::BTreeSet::new();
        let n = 40;
        for ix in 0..n {
            for iy in 0..n {
                let ndc = (
                    (ix as f32 / (n - 1) as f32) * 2.0 - 1.0,
                    (iy as f32 / (n - 1) as f32) * 2.0 - 1.0,
                );
                if let Some(f) = pick_face(&orbit, 1.3, ndc) {
                    seen.insert(f);
                }
            }
        }
        use cube_core::Face::*;
        assert_eq!(
            seen.into_iter().collect::<Vec<_>>(),
            vec![U, R, F],
            "visible faces from the default camera"
        );
    }
}
