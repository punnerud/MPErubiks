# Rubik's Cube — solver & trainer

A visual-first Rubik's cube app in Rust: scan your real cube with the
camera, watch it in 3D, and follow a step-by-step guide that solves it in
**at most 21 moves**. Train the standard algorithm sets (PLL · OLL · F2L ·
beginner) with a timer and per-algorithm scores — and once you know an
algorithm, the solver *prefers solutions that use it* and tells you
"★ T-Perm — you know this bit!".

Designed so children who can't read yet can use it: big colored buttons,
painter-drawn icons, animations and color cues carry the meaning; text is
always secondary. English/Norwegian with a flag switch.

## Targets

- **Web (primary):** `trunk serve` → WebGPU with automatic WebGL2 fallback.
  Camera scanning needs HTTPS or localhost (`trunk serve --tls` for phones).
- **Native desktop:** `cargo run -p cube-app` (same UI, manual cube entry
  instead of camera).

## Layout

| crate | what |
|---|---|
| `cube-core` | dependency-free cube model: 54-facelet state, moves derived from integer sticker geometry, notation parser, case recognizer (patterns derived from algorithm inverses) |
| `cube-solver` | kewb (Kociemba two-phase) integration, ≤21-move bound, validation, hint engine |
| `cube-vision` | Oklab color classification of camera patches, center-based recalibration |
| `cube-render` | wgpu: instanced cubies, procedural sticker shader, move animation, offscreen depth + blit into egui |
| `cube-store` | MPEdb storage (file+WAL native, in-memory + localStorage dump on wasm) |
| `cube-app` | eframe/egui app: menu, play, scan, solve guide, trainer |

`assets/algorithms.json` holds 127 validated cases — every algorithm is
checked mechanically (its inverse must produce a state of the claimed
kind, and no two cases may be AUF-indistinguishable), so data errors fail
the build. `assets/table.bin` is the kewb move/pruning table
(`cargo run -p xtask -- gen-table` regenerates it).

## Tests

```sh
cargo test --workspace              # core properties, recognizer, vision, store
cargo test -p cube-solver -- --ignored   # 1000 random solves ≤ 21/23 moves
UPDATE_SNAPSHOTS=true cargo test -p cube-app --test screenshots  # headless UI PNGs
```

The screenshot suite renders real app screens through wgpu (lavapipe
works) into `crates/cube-app/tests/snapshots/`.
