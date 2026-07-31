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

    pub fn eye(&self) -> [f32; 3] {
        [
            self.dist * self.pitch.cos() * self.yaw.sin(),
            self.dist * self.pitch.sin(),
            self.dist * self.pitch.cos() * self.yaw.cos(),
        ]
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = look_at(self.eye(), [0.0; 3], [0.0, 1.0, 0.0]);
        let proj = perspective(45f32.to_radians(), aspect.max(0.01), 0.1, 100.0);
        mat4_mul(proj, view)
    }

    /// World-space ray through a point in normalized device coords
    /// (-1..1 each axis, +y up). Matches the `view_proj` projection.
    pub fn ray(&self, aspect: f32, ndc: (f32, f32)) -> ([f32; 3], [f32; 3]) {
        let eye = self.eye();
        let fwd = normalize3([-eye[0], -eye[1], -eye[2]]);
        let right = normalize3(cross3(fwd, [0.0, 1.0, 0.0]));
        let up = cross3(right, fwd);
        let tan = (45f32.to_radians() / 2.0).tan();
        let dir = normalize3([
            fwd[0] + ndc.0 * tan * aspect.max(0.01) * right[0] + ndc.1 * tan * up[0],
            fwd[1] + ndc.0 * tan * aspect.max(0.01) * right[1] + ndc.1 * tan * up[1],
            fwd[2] + ndc.0 * tan * aspect.max(0.01) * right[2] + ndc.1 * tan * up[2],
        ]);
        (eye, dir)
    }
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / len, v[1] / len, v[2] / len]
}
