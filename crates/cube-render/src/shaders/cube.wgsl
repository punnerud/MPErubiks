// Cubie rendering: instanced unit cubes, procedural sticker borders,
// two soft directional lights. WebGL2-safe (no storage buffers).

struct Globals {
    view_proj: mat4x4<f32>,
    misc: vec4<f32>, // x = time (s)
};
@group(0) @binding(0) var<uniform> globals: Globals;

var<private> PALETTE: array<vec4<f32>, 8> = array<vec4<f32>, 8>(
    vec4<f32>(0.97, 0.97, 0.97, 1.0), // 0 U white
    vec4<f32>(0.88, 0.11, 0.18, 1.0), // 1 R red
    vec4<f32>(0.00, 0.66, 0.38, 1.0), // 2 F green
    vec4<f32>(1.00, 0.84, 0.00, 1.0), // 3 D yellow
    vec4<f32>(1.00, 0.38, 0.00, 1.0), // 4 L orange
    vec4<f32>(0.05, 0.36, 0.78, 1.0), // 5 B blue
    vec4<f32>(0.07, 0.07, 0.08, 1.0), // 6 interior plastic
    vec4<f32>(0.58, 0.58, 0.62, 1.0), // 7 unknown (scan preview)
);

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) face_id: u32,
    @location(4) i_pos: vec3<f32>,
    @location(5) i_rot: vec4<f32>,
    @location(6) i_colors: u32,
    @location(7) i_flags: u32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_nrm: vec3<f32>,
    @location(2) @interpolate(flat) color_idx: u32,
    @location(3) @interpolate(flat) flags: u32,
};

fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

@vertex
fn vs(in: VsIn) -> VsOut {
    // Rotate the whole cubie (vertex + its grid offset) about the cube
    // center so a turning layer pivots correctly.
    let world = quat_rotate(in.i_rot, in.pos + in.i_pos);
    var out: VsOut;
    out.clip = globals.view_proj * vec4<f32>(world, 1.0);
    out.uv = in.uv;
    out.world_nrm = quat_rotate(in.i_rot, in.nrm);
    out.color_idx = (in.i_colors >> (4u * in.face_id)) & 0xFu;
    out.flags = in.i_flags;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let border = 0.055;
    let edge = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
    var sticker = smoothstep(border, border + 0.025, edge);
    if in.color_idx == 6u {
        sticker = 0.0; // interior faces are all plastic
    }
    let plastic = vec3<f32>(0.07, 0.07, 0.08);
    var c = mix(plastic, PALETTE[in.color_idx].rgb, sticker);
    if (in.flags & 1u) != 0u {
        // Next-move layer: gentle white pulse so kids see what turns next.
        let pulse = 0.22 + 0.16 * sin(globals.misc.x * 6.0);
        c = mix(c, vec3<f32>(1.0, 1.0, 1.0), pulse * sticker);
    }
    let n = normalize(in.world_nrm);
    let l1 = max(dot(n, normalize(vec3<f32>(0.45, 0.8, 0.55))), 0.0);
    let l2 = max(dot(n, normalize(vec3<f32>(-0.5, 0.25, -0.6))), 0.0);
    let light = 0.45 + 0.48 * l1 + 0.18 * l2;
    return vec4<f32>(c * light, 1.0);
}
