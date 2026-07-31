//! Read-only piece (cubie) view over the facelet state: locate a corner or
//! edge piece by its sticker colors. Slot ids and orientations are internal
//! conventions (stable, derived from geometry) — they only need to be
//! consistent, not to match any external numbering.

use crate::facelet::{geom, Color, FaceletCube};

impl FaceletCube {
    /// Sticker colors currently sitting in corner slot `slot` (0..8), in the
    /// slot's canonical facelet order (U/D facelet first).
    pub fn corner_colors(&self, slot: u8) -> [Color; 3] {
        let t = geom().corner_slots[slot as usize];
        [
            self.0[t[0] as usize],
            self.0[t[1] as usize],
            self.0[t[2] as usize],
        ]
    }

    /// Sticker colors in edge slot `slot` (0..12).
    pub fn edge_colors(&self, slot: u8) -> [Color; 2] {
        let p = geom().edge_slots[slot as usize];
        [self.0[p[0] as usize], self.0[p[1] as usize]]
    }

    /// Find the corner piece with exactly these three colors. Returns
    /// `(slot, orientation)` where `orientation` is the position of
    /// `colors[0]` within the slot's canonical facelet order.
    pub fn locate_corner(&self, colors: [Color; 3]) -> Option<(u8, u8)> {
        let want = sorted3(colors);
        for slot in 0..8u8 {
            let have = self.corner_colors(slot);
            if sorted3(have) == want {
                let ori = have.iter().position(|&c| c == colors[0])? as u8;
                return Some((slot, ori));
            }
        }
        None
    }

    /// Find the edge piece with exactly these two colors. Returns
    /// `(slot, orientation)` where `orientation` is the position of
    /// `colors[0]` within the slot's canonical facelet order.
    pub fn locate_edge(&self, colors: [Color; 2]) -> Option<(u8, u8)> {
        let want = sorted2(colors);
        for slot in 0..12u8 {
            let have = self.edge_colors(slot);
            if sorted2(have) == want {
                let ori = have.iter().position(|&c| c == colors[0])? as u8;
                return Some((slot, ori));
            }
        }
        None
    }
}

fn sorted3(mut c: [Color; 3]) -> [Color; 3] {
    c.sort_unstable();
    c
}

fn sorted2(mut c: [Color; 2]) -> [Color; 2] {
    c.sort_unstable();
    c
}
