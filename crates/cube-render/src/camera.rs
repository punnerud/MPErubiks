use crate::math::{look_at, mat4_mul, perspective, Mat4};

/// Orbit camera around the cube center: drag to rotate, scroll to zoom.
#[derive(Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        // Slightly from above-right so three faces are visible.
        OrbitCamera {
            yaw: 0.6,
            pitch: 0.45,
            dist: 9.0,
        }
    }
}

impl OrbitCamera {
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(-1.4, 1.4);
    }

    pub fn zoom(&mut self, scroll: f32) {
        self.dist = (self.dist * (1.0 - scroll * 0.001)).clamp(5.0, 16.0);
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let eye = [
            self.dist * self.pitch.cos() * self.yaw.sin(),
            self.dist * self.pitch.sin(),
            self.dist * self.pitch.cos() * self.yaw.cos(),
        ];
        let view = look_at(eye, [0.0; 3], [0.0, 1.0, 0.0]);
        let proj = perspective(45f32.to_radians(), aspect.max(0.01), 0.1, 100.0);
        mat4_mul(proj, view)
    }
}
