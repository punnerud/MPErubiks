//! Move animation as pure poll-based logic (no wgpu, unit-testable).
//! The app calls `tick` every frame, applies the completed moves to its
//! `FaceletCube`, and feeds `pose` into the instance builder.

use cube_core::{Face, Move, RotAxis, SliceKind, Turns};
use std::collections::VecDeque;

/// Which cubies are mid-rotation, about which axis, by how much.
#[derive(Clone, Copy, Debug)]
pub struct LayerPose {
    /// Unit rotation axis.
    pub axis: [f32; 3],
    /// Current signed angle in radians (eased).
    pub angle: f32,
    /// Bit per cubie grid index (27 bits).
    pub mask: u32,
}

struct Active {
    mv: Move,
    start: f64,
}

pub struct MoveAnimator {
    queue: VecDeque<Move>,
    active: Option<Active>,
    pub secs_per_quarter: f32,
}

impl Default for MoveAnimator {
    fn default() -> Self {
        MoveAnimator {
            queue: VecDeque::new(),
            active: None,
            secs_per_quarter: 0.25,
        }
    }
}

impl MoveAnimator {
    pub fn enqueue(&mut self, mv: Move) {
        self.queue.push_back(mv);
    }

    pub fn enqueue_all<'a>(&mut self, moves: impl IntoIterator<Item = &'a Move>) {
        for &m in moves {
            self.queue.push_back(m);
        }
    }

    /// Drop everything, including any in-flight move (which is NOT applied).
    pub fn clear(&mut self) {
        self.queue.clear();
        self.active = None;
    }

    pub fn is_idle(&self) -> bool {
        self.active.is_none() && self.queue.is_empty()
    }

    pub fn pending(&self) -> usize {
        self.queue.len() + usize::from(self.active.is_some())
    }

    fn duration(&self, mv: Move) -> f64 {
        // Half turns take a bit MORE than two quarters: they animate in
        // two visible stages (turn, tiny rest at 90°, turn) so a double
        // is unmistakable from a single without reading the notation.
        let quarters = match turns_of(mv) {
            Turns::Half => 2.4,
            _ => 1.0,
        };
        f64::from(self.secs_per_quarter) * quarters
    }

    /// Advance to `now` (seconds); returns the moves that completed this
    /// tick, in order — apply each to the cube state.
    pub fn tick(&mut self, now: f64) -> Vec<Move> {
        let mut completed = Vec::new();
        loop {
            match &self.active {
                None => match self.queue.pop_front() {
                    Some(mv) => self.active = Some(Active { mv, start: now }),
                    None => break,
                },
                Some(a) => {
                    let end = a.start + self.duration(a.mv);
                    if now < end {
                        break;
                    }
                    completed.push(a.mv);
                    self.active = None;
                    // Keep rhythm unless we stalled badly.
                    let next_start = if now - end > 0.25 { now } else { end };
                    if let Some(mv) = self.queue.pop_front() {
                        self.active = Some(Active {
                            mv,
                            start: next_start,
                        });
                    }
                }
            }
        }
        completed
    }

    pub fn pose(&self, now: f64) -> Option<LayerPose> {
        let a = self.active.as_ref()?;
        let t = ((now - a.start) / self.duration(a.mv)).clamp(0.0, 1.0) as f32;
        let ease = |t: f32| {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        };
        let eased = if matches!(turns_of(a.mv), Turns::Half) {
            // Two-stage: 0..90° | rest | 90..180° — the eye counts "two".
            if t < 0.44 {
                0.5 * ease(t / 0.44)
            } else if t < 0.56 {
                0.5
            } else {
                0.5 + 0.5 * ease((t - 0.56) / 0.44)
            }
        } else {
            ease(t)
        };
        let (dir, signed_quarters, _) = move_dir(a.mv);
        let axis = [f32::from(dir[0]), f32::from(dir[1]), f32::from(dir[2])];
        Some(LayerPose {
            axis,
            // Cw is a -90° right-hand rotation about the oriented axis.
            angle: -std::f32::consts::FRAC_PI_2 * f32::from(signed_quarters) * eased,
            mask: crate::scene::mask_for_move(a.mv),
        })
    }

    /// The move currently animating or next in line (for highlighting).
    pub fn upcoming(&self) -> Option<Move> {
        self.active
            .as_ref()
            .map(|a| a.mv)
            .or_else(|| self.queue.front().copied())
    }
}

fn turns_of(mv: Move) -> Turns {
    match mv {
        Move::Face(_, t) | Move::Wide(_, t) | Move::Slice(_, t) | Move::Rot(_, t) => t,
    }
}

pub(crate) enum LayerSel {
    Face,
    Wide,
    Mid,
    All,
}

/// (oriented axis as int vector, signed quarter turns, layer selector).
pub(crate) fn move_dir(mv: Move) -> ([i8; 3], i8, LayerSel) {
    let face_dir = |f: Face| -> [i8; 3] {
        match f {
            Face::U => [0, 1, 0],
            Face::R => [1, 0, 0],
            Face::F => [0, 0, 1],
            Face::D => [0, -1, 0],
            Face::L => [-1, 0, 0],
            Face::B => [0, 0, -1],
        }
    };
    let signed = |t: Turns| -> i8 {
        match t {
            Turns::Cw => 1,
            Turns::Half => 2,
            Turns::Ccw => -1,
        }
    };
    match mv {
        Move::Face(f, t) => (face_dir(f), signed(t), LayerSel::Face),
        Move::Wide(f, t) => (face_dir(f), signed(t), LayerSel::Wide),
        Move::Slice(SliceKind::M, t) => ([-1, 0, 0], signed(t), LayerSel::Mid),
        Move::Slice(SliceKind::E, t) => ([0, -1, 0], signed(t), LayerSel::Mid),
        Move::Slice(SliceKind::S, t) => ([0, 0, 1], signed(t), LayerSel::Mid),
        Move::Rot(RotAxis::X, t) => ([1, 0, 0], signed(t), LayerSel::All),
        Move::Rot(RotAxis::Y, t) => ([0, 1, 0], signed(t), LayerSel::All),
        Move::Rot(RotAxis::Z, t) => ([0, 0, 1], signed(t), LayerSel::All),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_core::Alg;

    #[test]
    fn queue_completes_in_order() {
        let mut anim = MoveAnimator::default();
        anim.enqueue_all(&Alg::parse("R U2 F'").unwrap().0);
        assert_eq!(anim.pending(), 3);
        assert!(anim.tick(0.0).is_empty()); // starts R
        assert!(anim.pose(0.1).is_some());
        let done = anim.tick(0.26); // R done (0.25s), U2 starts
        assert_eq!(done, Alg::parse("R").unwrap().0);
        let done = anim.tick(0.80); // U2 takes 0.5s: done at 0.75
        assert_eq!(done, Alg::parse("U2").unwrap().0);
        let done = anim.tick(5.0);
        assert_eq!(done, Alg::parse("F'").unwrap().0);
        assert!(anim.is_idle());
    }

    #[test]
    fn stalled_frame_resumes_gracefully() {
        // A long stall (backgrounded tab): the in-flight move completes,
        // the REST resume animating from 'now' — never an invisible
        // fast-forward of the whole queue.
        let mut anim = MoveAnimator::default();
        anim.enqueue_all(&Alg::parse("R U R' U'").unwrap().0);
        anim.tick(0.0);
        assert_eq!(anim.tick(100.0), Alg::parse("R").unwrap().0);
        assert!(anim.pose(100.1).is_some(), "U animates after the stall");
        assert_eq!(anim.tick(100.3), Alg::parse("U").unwrap().0);
        assert_eq!(anim.tick(100.6), Alg::parse("R'").unwrap().0);
        assert_eq!(anim.tick(100.8), Alg::parse("U'").unwrap().0);
        assert!(anim.is_idle());
    }

    #[test]
    fn masks_select_expected_cubie_counts() {
        let count = |s: &str| {
            crate::scene::mask_for_move(Alg::parse(s).unwrap().0[0]).count_ones()
        };
        assert_eq!(count("R"), 9);
        assert_eq!(count("Rw"), 18);
        assert_eq!(count("M"), 9);
        assert_eq!(count("x"), 27);
        assert_eq!(count("U'"), 9);
        assert_eq!(count("E2"), 9);
    }

    #[test]
    fn pose_angle_reaches_target() {
        let mut anim = MoveAnimator::default();
        anim.enqueue(Alg::parse("R").unwrap().0[0]);
        anim.tick(0.0);
        let p = anim.pose(0.2499).unwrap();
        assert!(p.angle < -1.5, "angle close to -pi/2, got {}", p.angle);
        let mut anim2 = MoveAnimator::default();
        anim2.enqueue(Alg::parse("R'").unwrap().0[0]);
        anim2.tick(0.0);
        assert!(anim2.pose(0.2).unwrap().angle > 0.0);
    }
}
