//! Tap-to-select: cast a ray from the camera through the tapped pixel and
//! find which face layer the hit sticker belongs to. Pure math, testable.

use crate::camera::OrbitCamera;
use crate::scene::CUBIE_SPACING;
use cube_core::{Face, Turns};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArrowDir {
    Left,
    Right,
    Up,
    Down,
}

/// View-relative turn arrows for a selected layer: the part of the layer
/// NEAREST the viewer moves in the arrow's screen direction. Returns two
/// (arrow, turn) pairs — horizontal (←/→) or vertical (↑/↓), whichever
/// dominates on screen for this face and camera.
pub fn view_relative_arrows(orbit: &OrbitCamera, face: Face) -> [(ArrowDir, Turns); 2] {
    let d = match face {
        Face::U => [0.0f32, 1.0, 0.0],
        Face::D => [0.0, -1.0, 0.0],
        Face::R => [1.0, 0.0, 0.0],
        Face::L => [-1.0, 0.0, 0.0],
        Face::F => [0.0, 0.0, 1.0],
        Face::B => [0.0, 0.0, -1.0],
    };
    let eye = orbit.eye();
    // Nearest point of the layer's ring to the eye: the eye projected
    // onto the layer plane (fallback to camera-right when degenerate).
    let along = eye[0] * d[0] + eye[1] * d[1] + eye[2] * d[2];
    let mut p = [
        eye[0] - along * d[0],
        eye[1] - along * d[1],
        eye[2] - along * d[2],
    ];
    let (_, right, up) = orbit.basis();
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    if len < 1e-3 {
        p = right;
    } else {
        p = [p[0] / len, p[1] / len, p[2] / len];
    }
    let ring = CUBIE_SPACING;
    let p = [p[0] * ring, p[1] * ring, p[2] * ring];
    // Cw is a -90° right-hand rotation about d: angular velocity = -d.
    let v_cw = [
        -(d[1] * p[2] - d[2] * p[1]),
        -(d[2] * p[0] - d[0] * p[2]),
        -(d[0] * p[1] - d[1] * p[0]),
    ];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let vr = dot(v_cw, right);
    let vu = dot(v_cw, up);
    if vr.abs() >= vu.abs() {
        let right_turn = if vr > 0.0 { Turns::Cw } else { Turns::Ccw };
        [
            (ArrowDir::Left, right_turn.inverse()),
            (ArrowDir::Right, right_turn),
        ]
    } else {
        let up_turn = if vu > 0.0 { Turns::Cw } else { Turns::Ccw };
        [(ArrowDir::Up, up_turn), (ArrowDir::Down, up_turn.inverse())]
    }
}

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
    fn view_arrows_are_consistent() {
        let orbit = OrbitCamera::default();
        // U spins horizontally on screen; R mostly vertically.
        let u = view_relative_arrows(&orbit, cube_core::Face::U);
        assert!(matches!(u[0].0, ArrowDir::Left) && matches!(u[1].0, ArrowDir::Right));
        assert_eq!(u[0].1, u[1].1.inverse(), "arrows are opposite turns");
        let r = view_relative_arrows(&orbit, cube_core::Face::R);
        assert!(
            matches!(r[0].0, ArrowDir::Up),
            "R layer moves vertically from the default view, got {:?}",
            r[0].0
        );
        // Walking around a vertical-axis layer does NOT flip the apparent
        // direction (near point and screen-right both negate — they
        // cancel). The mapping must be stable.
        let mut opposite = OrbitCamera::default();
        opposite.yaw += std::f32::consts::PI;
        let u2 = view_relative_arrows(&opposite, cube_core::Face::U);
        assert_eq!(u2[1].1, u[1].1, "walking behind keeps the same mapping");
    }

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
