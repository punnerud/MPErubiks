//! The eframe application shell: screen routing, async plumbing, top bar.

use crate::i18n::{I18n, Lang, TextKey};
use crate::library::Library;
use crate::screens::{self, solve::SolveScreen, train::{Attempt, TrainScreen}};
use std::collections::{HashMap, HashSet};
use cube_core::{FaceletCube, Move, SplitMix64, Turns};
use cube_render::{CubeRenderResources, MoveAnimator, OrbitCamera};
use std::sync::mpsc::{Receiver, Sender};

pub enum Screen {
    Menu,
    Play,
    Solve(SolveScreen),
    Train(TrainScreen),
    #[cfg(target_arch = "wasm32")]
    Scan(crate::screens::scan::ScanScreen),
}

pub enum TableState {
    Loading,
    Ready,
    Failed(String),
}

/// Messages from async tasks (wasm fetch, camera init) into the UI loop.
pub enum AsyncMsg {
    TableBytes(Result<Vec<u8>, String>),
    #[cfg(target_arch = "wasm32")]
    CameraReady(Result<crate::platform::web::camera::Camera, String>),
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
    pub table: TableState,
    pub library: Library,
    /// Cases the user marks as "trained" (drives solver hints).
    pub trained: HashSet<u16>,
    /// Training attempts per case (persisted via cube-store in M6).
    pub attempts: HashMap<u16, Vec<Attempt>>,
    pub store: Option<cube_store::Store>,
    pub tx: Sender<AsyncMsg>,
    rx: Receiver<AsyncMsg>,
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
        let (tx, rx) = std::sync::mpsc::channel();
        let seed = (cc.egui_ctx.input(|i| i.time).to_bits()).wrapping_mul(0x9E37_79B9);

        let table = install_table_for_platform(&cc.egui_ctx, &tx);
        let store = crate::persist::open_store();

        let mut app = RubiksApp {
            screen: Screen::Menu,
            i18n: I18n { lang: Lang::En },
            cube: FaceletCube::SOLVED,
            animator: MoveAnimator::default(),
            orbit: OrbitCamera::default(),
            rng: SplitMix64::new(seed | 1),
            history: Vec::new(),
            table,
            library: Library::load(),
            trained: HashSet::new(),
            attempts: HashMap::new(),
            store,
            tx,
            rx,
        };
        crate::persist::warm_app(&mut app);
        app
    }

    /// Record a training attempt (in-memory + database + wasm mirror).
    pub fn record_attempt(&mut self, case_idx: u16, ms: u64, success: bool) {
        self.attempts
            .entry(case_idx)
            .or_default()
            .push(crate::screens::train::Attempt { ms, success });
        if let Some(store) = &self.store {
            let id = self.library.rec.case(case_idx).id.clone();
            if let Err(e) =
                store.record_result(&id, crate::persist::now_ms(), ms.max(1) as i64, success)
            {
                log::error!("record: {e}");
            }
            crate::persist::persist(store);
        }
    }

    pub fn toggle_trained(&mut self, case_idx: u16) {
        let trained = if self.trained.contains(&case_idx) {
            self.trained.remove(&case_idx);
            false
        } else {
            self.trained.insert(case_idx);
            true
        };
        if let Some(store) = &self.store {
            let id = self.library.rec.case(case_idx).id.clone();
            if let Err(e) = store.set_trained(&id, trained) {
                log::error!("set_trained: {e}");
            }
            crate::persist::persist(store);
        }
    }

    pub fn set_lang(&mut self, lang: crate::i18n::Lang) {
        self.i18n.lang = lang;
        if let Some(store) = &self.store {
            let code = match lang {
                crate::i18n::Lang::En => "en",
                crate::i18n::Lang::No => "no",
            };
            if let Err(e) = store.set_setting("lang", code) {
                log::error!("set lang: {e}");
            }
            crate::persist::persist(store);
        }
    }

    pub fn t(&self, k: TextKey) -> &'static str {
        self.i18n.t(k)
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

    fn drain_async(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AsyncMsg::TableBytes(Ok(bytes)) => {
                    self.table = match cube_solver::install_table(&bytes) {
                        Ok(()) => TableState::Ready,
                        Err(e) => TableState::Failed(e.to_string()),
                    };
                }
                AsyncMsg::TableBytes(Err(e)) => {
                    self.table = TableState::Failed(e);
                }
                #[cfg(target_arch = "wasm32")]
                AsyncMsg::CameraReady(result) => {
                    if let Screen::Scan(scan) = &mut self.screen {
                        scan.set_camera(result);
                    } else if let Ok(cam) = result {
                        cam.stop(); // user left the scan screen meanwhile
                    }
                }
            }
        }
    }
}

impl eframe::App for RubiksApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let _ = &frame;
        self.drain_async();
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
                Screen::Solve(_) => screens::solve::show(self, ui),
                Screen::Train(_) => screens::train::show(self, ui),
                #[cfg(target_arch = "wasm32")]
                Screen::Scan(_) => screens::scan::show(self, ui, frame),
            });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn install_table_for_platform(
    _ctx: &egui::Context,
    _tx: &Sender<AsyncMsg>,
) -> TableState {
    // Native: the table ships inside the binary.
    static TABLE_BYTES: &[u8] =
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/table.bin"));
    match cube_solver::install_table(TABLE_BYTES) {
        Ok(()) => TableState::Ready,
        Err(e) => TableState::Failed(e.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
fn install_table_for_platform(ctx: &egui::Context, tx: &Sender<AsyncMsg>) -> TableState {
    // Browser: fetch as a separate asset (cached independently of the wasm).
    let ctx = ctx.clone();
    let tx = tx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let result = crate::platform::web::fetch_bytes("table.bin").await;
        let _ = tx.send(AsyncMsg::TableBytes(result));
        ctx.request_repaint();
    });
    TableState::Loading
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
