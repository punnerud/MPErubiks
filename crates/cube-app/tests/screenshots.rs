//! Headless screenshot tests: render real app screens via wgpu (lavapipe
//! in CI/dev boxes) and snapshot them to PNG. Run with
//! `UPDATE_SNAPSHOTS=true cargo test -p cube-app --test screenshots`
//! to (re)generate baselines in tests/snapshots/.

use cube_app::app::{RubiksApp, Screen};
use egui_kittest::Harness;

fn harness<'a>() -> Harness<'a, RubiksApp> {
    Harness::builder()
        .with_size(egui::Vec2::new(1024.0, 768.0))
        .wgpu()
        .build_eframe(|cc| RubiksApp::new(cc))
}

#[test]
fn menu_screen() {
    let mut h = harness();
    h.run();
    h.snapshot("menu");
}

#[test]
fn play_screen_solved() {
    let mut h = harness();
    h.state_mut().screen = Screen::Play;
    h.run();
    h.snapshot("play_solved");
}

#[test]
fn play_screen_scrambled() {
    let mut h = harness();
    h.state_mut().screen = Screen::Play;
    // Apply a fixed scramble instantly (no animation) so the snapshot is
    // deterministic and shows a colorful mixed cube.
    let alg = cube_core::Alg::parse("R U F2 L' D B U2 R' F L2 D' B2").unwrap();
    h.state_mut().cube.apply_alg(&alg);
    h.run();
    h.snapshot("play_scrambled");
}

#[test]
fn menu_norwegian() {
    let mut h = harness();
    h.state_mut().i18n.lang = cube_app::i18n::Lang::No;
    h.run();
    h.snapshot("menu_no");
}
