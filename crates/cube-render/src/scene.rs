//! Cubie instances: mapping cube state + animation pose onto the GPU
//! instance buffer.

use crate::animator::{move_dir, LayerPose, LayerSel};
use crate::math::{quat_axis_angle, QUAT_IDENTITY};
use cube_core::{sticker_index, Face, FaceletCube, Move};
use std::sync::OnceLock;

/// Palette index for interior (sticker-less) cubie faces.
pub const COLOR_INTERIOR: u32 = 6;
/// Palette index for "unknown" stickers (scan preview).
pub const COLOR_UNKNOWN: u32 = 7;

/// Spacing between cubie centers (cubie mesh is a unit cube).
pub const CUBIE_SPACING: f32 = 1.04;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub pos: [f32; 3],
    pub _pad0: f32,
    /// Rotation quaternion (layer animation; identity when at rest).
    pub rot: [f32; 4],
    /// 6 palette indices, 4 bits each, in Face order U R F D L B.
    pub colors: u32,
    /// bit 0: highlighted (next-move layer pulse).
    pub flags: u32,
    pub _pad1: [u32; 2],
}

fn grid_coord(i: usize) -> [i8; 3] {
    [(i / 9) as i8 - 1, ((i / 3) % 3) as i8 - 1, (i % 3) as i8 - 1]
}

/// facelet index per (cubie grid index, face), cached.
fn sticker_table() -> &'static [[Option<u8>; 6]; 27] {
    static TABLE: OnceLock<[[Option<u8>; 6]; 27]> = OnceLock::new();
    TABLE.get_or_init(|| {
        core::array::from_fn(|i| {
            let q = grid_coord(i);
            core::array::from_fn(|f| {
                sticker_index(q, Face::from_index(f)).map(|idx| idx as u8)
            })
        })
    })
}

/// Which cubies a move rotates, as a 27-bit mask over grid indices.
pub fn mask_for_move(mv: Move) -> u32 {
    let (dir, _, layer) = move_dir(mv);
    let axis = dir.iter().position(|&c| c != 0).expect("nonzero dir");
    let sign = dir[axis];
    let mut mask = 0u32;
    for i in 0..27 {
        let c = grid_coord(i)[axis] * sign;
        let in_layer = match layer {
            LayerSel::Face => c == 1,
            LayerSel::Wide => c >= 0,
            LayerSel::Mid => c == 0,
            LayerSel::All => true,
        };
        if in_layer {
            mask |= 1 << i;
        }
    }
    mask
}

/// Build the 27 instances for a state. `color_override` lets scan/preview
/// screens replace sticker colors (return `None` to use the state's color).
pub fn instances_from_state(
    state: &FaceletCube,
    pose: Option<&LayerPose>,
    highlight_mask: u32,
    color_override: Option<&dyn Fn(usize) -> Option<u32>>,
) -> [Instance; 27] {
    let table = sticker_table();
    core::array::from_fn(|i| {
        let q = grid_coord(i);
        let mut colors = 0u32;
        for f in 0..6 {
            let palette = match table[i][f] {
                Some(facelet) => {
                    let facelet = facelet as usize;
                    match color_override.and_then(|ov| ov(facelet)) {
                        Some(c) => c,
                        None => state.0[facelet] as u32,
                    }
                }
                None => COLOR_INTERIOR,
            };
            colors |= palette << (4 * f);
        }
        let rot = match pose {
            Some(p) if p.mask & (1 << i) != 0 => quat_axis_angle(p.axis, p.angle),
            _ => QUAT_IDENTITY,
        };
        let highlighted = highlight_mask & (1 << i) != 0;
        // When a selection exists, everything OUTSIDE it dims to grayscale
        // so the chosen layer visibly pops (bit1 = dimmed).
        let dimmed = highlight_mask != 0 && !highlighted;
        Instance {
            pos: [
                f32::from(q[0]) * CUBIE_SPACING,
                f32::from(q[1]) * CUBIE_SPACING,
                f32::from(q[2]) * CUBIE_SPACING,
            ],
            _pad0: 0.0,
            rot,
            colors,
            flags: u32::from(highlighted) | (u32::from(dimmed) << 1),
            _pad1: [0, 0],
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_surface_cubie_has_expected_sticker_count() {
        let table = sticker_table();
        let mut corner = 0;
        let mut edge = 0;
        let mut center = 0;
        let mut core_ = 0;
        for row in table.iter() {
            match row.iter().flatten().count() {
                3 => corner += 1,
                2 => edge += 1,
                1 => center += 1,
                0 => core_ += 1,
                n => panic!("cubie with {n} stickers"),
            }
        }
        assert_eq!((corner, edge, center, core_), (8, 12, 6, 1));
    }

    #[test]
    fn solved_instances_show_face_colors() {
        let inst = instances_from_state(&FaceletCube::SOLVED, None, 0, None);
        // The top-center cubie (grid (0,1,0)) must show U color (0) on U.
        let top_center = 1 * 9 + 2 * 3 + 1;
        let colors = inst[top_center].colors;
        assert_eq!(colors & 0xF, 0, "U face shows U color");
        assert_eq!((colors >> (4 * 3)) & 0xF, COLOR_INTERIOR, "D side of top cubie is interior");
    }
}
