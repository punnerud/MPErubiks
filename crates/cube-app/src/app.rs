//! The eframe application shell: screen routing, top bar, kid-friendly menu.

use crate::i18n::{I18n, Lang, TextKey};
use crate::screens;
use cube_core::{FaceletCube, Move, SplitMix64, Turns};
use cube_render::{CubeRenderResources, MoveAnimator, OrbitCamera};

pub enum Screen {
    Menu,
    Play,
    Solve,
    Train,
}

pub struct RubiksApp {
    pub screen: Screen,
    pub i18n: I18n,
    pub cube: FaceletCube,
    pub animator: MoveAnimator,
    pub orbit: OrbitCamera,
    pub rng: SplitMix64,
    /// Play-mode move history for undo.
    pub history: Vec<Move>,
}

impl RubiksApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(rs) = cc.wgpu_render_state.as_ref() {
            rs.renderer
                .write()
                .callback_resources
                .insert(CubeRenderResources::new(&rs.device, rs.target_format));
        }
        style(&cc.egui_ctx);
        let seed = (cc.egui_ctx.input(|i| i.time).to_bits()).wrapping_mul(0x9E37_79B9);
        RubiksApp {
            screen: Screen::Menu,
            i18n: I18n { lang: Lang::En },
            cube: FaceletCube::SOLVED,
            animator: MoveAnimator::default(),
            orbit: OrbitCamera::default(),
            rng: SplitMix64::new(seed | 1),
            history: Vec::new(),
        }
    }

    pub fn t(&self, k: TextKey) -> &'static str {
        self.i18n.t(k)
    }

    /// Queue a whole algorithm for animation.
    pub fn play_moves<'a>(&mut self, moves: impl IntoIterator<Item = &'a Move>) {
        self.animator.enqueue_all(moves);
    }

    pub fn scramble(&mut self) {
        use cube_core::Face;
        self.animator.clear();
        let mut prev_face: Option<Face> = None;
        let faces = [Face::U, Face::R, Face::F, Face::D, Face::L, Face::B];
        let turns = [Turns::Cw, Turns::Half, Turns::Ccw];
        for _ in 0..20 {
            let f = loop {
                let f = faces[self.rng.below(6) as usize];
                if Some(f) != prev_face {
                    break f;
                }
            };
            prev_face = Some(f);
            let m = Move::Face(f, turns[self.rng.below(3) as usize]);
            self.history.push(m);
            self.animator.enqueue(m);
        }
    }
}

impl eframe::App for RubiksApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = ui.input(|i| i.time);
        for m in self.animator.tick(now) {
            self.cube.apply(m);
        }
        if !self.animator.is_idle() {
            ui.ctx().request_repaint();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| match self.screen {
                Screen::Menu => screens::menu::show(self, ui),
                Screen::Play => screens::play::show(self, ui),
                Screen::Solve => screens::placeholder::show(self, ui),
                Screen::Train => screens::placeholder::show(self, ui),
            });
    }
}

fn style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        // Big touch targets and readable text — used by kids on tablets.
        style.spacing.interact_size = egui::Vec2::new(48.0, 48.0);
        style.spacing.item_spacing = egui::Vec2::new(12.0, 12.0);
        style.spacing.button_padding = egui::Vec2::new(16.0, 12.0);
        for (_, font) in style.text_styles.iter_mut() {
            font.size *= 1.25;
        }
        style.visuals.panel_fill = egui::Color32::from_rgb(0x12, 0x15, 0x1D);
    });
}
