//! Headless screenshot tests: render real app screens via wgpu (lavapipe
//! in CI/dev boxes) and snapshot them to PNG. Run with
//! `UPDATE_SNAPSHOTS=true cargo test -p cube-app --test screenshots`
//! to (re)generate baselines in tests/snapshots/.

use cube_app::app::{RubiksApp, Screen};
use egui_kittest::Harness;

fn harness<'a>() -> Harness<'a, RubiksApp> {
    // Isolate the store: never touch the user's real database from tests.
    std::env::set_var(
        "RUBIKS_DATA_DIR",
        std::env::temp_dir().join(format!("rubiks-test-{}", std::process::id())),
    );
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

#[test]
fn train_picker_pll() {
    let mut h = harness();
    h.state_mut().screen = Screen::Train(cube_app::screens::train::TrainScreen::Picker {
        tab: cube_app::screens::train::PickerTab::Set(cube_core::CaseSet::Pll),
    });
    h.run();
    h.snapshot("train_picker_pll");
}

#[test]
fn train_picker_oll() {
    let mut h = harness();
    h.state_mut().screen = Screen::Train(cube_app::screens::train::TrainScreen::Picker {
        tab: cube_app::screens::train::PickerTab::Set(cube_core::CaseSet::Oll),
    });
    h.run();
    h.snapshot("train_picker_oll");
}

#[test]
fn train_intro_tab() {
    let mut h = harness();
    h.state_mut().screen = Screen::Train(cube_app::screens::train::TrainScreen::Picker {
        tab: cube_app::screens::train::PickerTab::Intro,
    });
    h.run();
    h.snapshot("train_intro");
}

#[test]
fn lesson_daisy_phone() {
    // iPhone-ish viewport: the bug class this guards against is buttons
    // pushed past the right edge or cut by the bottom (URL-bar zone).
    std::env::set_var(
        "RUBIKS_DATA_DIR",
        std::env::temp_dir().join(format!("rubiks-test-{}", std::process::id())),
    );
    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(390.0, 740.0))
        .wgpu()
        .build_eframe(|cc| RubiksApp::new(cc));
    h.state_mut().screen = Screen::Train(cube_app::screens::train::TrainScreen::Lesson(
        cube_app::screens::train::LessonView { lesson: 2, step: 0, pending: true, demo_len: 0 },
    ));
    h.run_steps(3);
    h.snapshot("lesson_daisy_phone");
}

#[test]
fn lesson_daisy_screen() {
    let mut h = harness();
    h.state_mut().screen = Screen::Train(cube_app::screens::train::TrainScreen::Lesson(
        cube_app::screens::train::LessonView { lesson: 2, step: 0, pending: true, demo_len: 0 },
    ));
    h.step(); // applies the step's setup and queues the demo
    {
        // Jump to the demo's end state so the snapshot is deterministic
        // (a running animation would keep the harness stepping).
        let app = h.state_mut();
        app.animator.clear();
        let demo = cube_app::lessons::LESSONS[2].steps[0].demo.unwrap();
        app.cube.apply_alg(&cube_core::Alg::parse(demo).unwrap());
    }
    h.run();
    h.snapshot("lesson_daisy");
}

#[test]
fn train_session_ready() {
    let mut h = harness();
    {
        let app = h.state_mut();
        let t_idx = app.library.rec.find_by_id("pll-t").unwrap();
        let mut rng = cube_core::SplitMix64::new(42);
        app.cube = app.library.rec.setup_state(t_idx, &mut rng);
        app.screen = Screen::Train(cube_app::screens::train::TrainScreen::Session(
            cube_app::screens::train::SessionState {
                case_idx: t_idx,
                phase: cube_app::screens::train::Phase::Ready,
            },
        ));
    }
    h.run();
    h.snapshot("train_session_ready");
}

#[test]
fn solve_guide_with_trained_hint() {
    let mut h = harness();
    {
        let app = h.state_mut();
        let t_idx = app.library.rec.find_by_id("pll-t").unwrap();
        app.trained.insert(t_idx);
        // A pure T-perm state: the guided solution should be exactly the
        // trained algorithm, labeled with its name.
        let mut rng = cube_core::SplitMix64::new(3);
        let state = app.library.rec.setup_state(t_idx, &mut rng);
        let out =
            cube_solver::solve_with_hints(&state, &[t_idx], &app.library.rec).unwrap();
        assert!(out.guided.is_some(), "T-perm state must yield a guided solution");
        let guide = cube_app::screens::solve::GuideState::from_output(out, &app.library.rec);
        assert!(
            guide.labels.iter().any(|l| l.as_deref() == Some("T-Perm")),
            "guided solution must carry the T-Perm label"
        );
        app.cube = state;
        app.screen = Screen::Solve(cube_app::screens::solve::SolveScreen::Guide(guide));
    }
    h.run();
    h.snapshot("solve_guide_hint");
}

#[test]
fn solve_input_screen() {
    let mut h = harness();
    let scrambled = {
        let alg = cube_core::Alg::parse("R U F2 L' D B U2 R' F").unwrap();
        cube_core::FaceletCube::SOLVED.applied_alg(&alg)
    };
    h.state_mut().screen =
        Screen::Solve(cube_app::screens::solve::SolveScreen::new_input(scrambled));
    h.run();
    h.snapshot("solve_input");
}

#[test]
fn solve_guide_narrow_phone() {
    // iPhone-width viewport: every control must stay inside the screen.
    std::env::set_var(
        "RUBIKS_DATA_DIR",
        std::env::temp_dir().join(format!("rubiks-test-{}", std::process::id())),
    );
    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(390.0, 740.0))
        .wgpu()
        .build_eframe(|cc| RubiksApp::new(cc));
    let alg = cube_core::Alg::parse("R U F2 L' D B U2 R' F L2 D'").unwrap();
    let state = cube_core::FaceletCube::SOLVED.applied_alg(&alg);
    let solution = cube_solver::solve(&state).expect("table");
    {
        let app = h.state_mut();
        app.cube = state;
        app.screen = Screen::Solve(cube_app::screens::solve::SolveScreen::Guide(
            cube_app::screens::solve::GuideState::plain(solution),
        ));
    }
    h.run();
    h.snapshot("solve_guide_narrow");
}

#[test]
fn solve_guide_screen() {
    let mut h = harness();
    // Native harness installs the solver table at startup, so we can build
    // a real guide: scrambled state + its actual ≤21-move solution.
    let alg = cube_core::Alg::parse("R U F2 L' D B U2 R' F L2 D'").unwrap();
    let state = cube_core::FaceletCube::SOLVED.applied_alg(&alg);
    let solution = cube_solver::solve(&state).expect("solver table installed natively");
    assert!(solution.len_htm() <= 21, "got {} moves", solution.len_htm());
    {
        let app = h.state_mut();
        app.cube = state;
        app.screen = Screen::Solve(cube_app::screens::solve::SolveScreen::Guide(
            cube_app::screens::solve::GuideState::plain(solution),
        ));
    }
    h.run();
    h.snapshot("solve_guide");
}
