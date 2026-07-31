//! Minimal column-major mat4 / quaternion helpers (avoids a math-crate
//! dependency for the little this renderer needs).

pub type Mat4 = [[f32; 4]; 4]; // column-major: m[col][row]

pub fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut out = [[0.0f32; 4]; 4];
    for (c, out_col) in out.iter_mut().enumerate() {
        for (r, out_val) in out_col.iter_mut().enumerate() {
            *out_val = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

pub fn perspective(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y_rad / 2.0).tan();
    // wgpu clip space: z in 0..1.
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, (near * far) / (near - far), 0.0],
    ]
}

pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Mat4 {
    let fwd = normalize(sub(target, eye));
    let right = normalize(cross(fwd, up));
    let u = cross(right, fwd);
    [
        [right[0], u[0], -fwd[0], 0.0],
        [right[1], u[1], -fwd[1], 0.0],
        [right[2], u[2], -fwd[2], 0.0],
        [-dot(right, eye), -dot(u, eye), dot(fwd, eye), 1.0],
    ]
}

/// Quaternion (x, y, z, w) from a rotation about a unit axis.
pub fn quat_axis_angle(axis: [f32; 3], angle_rad: f32) -> [f32; 4] {
    let (s, c) = (angle_rad / 2.0).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

pub const QUAT_IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt();
    [v[0] / len, v[1] / len, v[2] / len]
}
