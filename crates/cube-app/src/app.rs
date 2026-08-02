//! The eframe application shell: screen routing, async plumbing, top bar.

use crate::i18n::{I18n, Lang, TextKey};
use crate::library::Library;
use crate::screens::{self, solve::SolveScreen, train::{Attempt, TrainScreen}};
use std::collections::{HashMap, HashSet};
use cube_core::{FaceletCube, Move, SplitMix64, Turns};
use cube_render::{CubeRenderResources, MoveAnimator, OrbitCamera};
use std::sync::mpsc::{Receiver, Sender};

/// Extra bottom breathing room: iOS Safari's collapsing URL bar overlays
/// the bottom of the viewport; buttons must sit above it.
pub const BOTTOM_INSET: f32 = 14.0;

pub enum Screen {
    Menu,
    Play,
    Solve(SolveScreen),
    Train(TrainScreen),
    Settings(crate::screens::settings::SettingsScreen),
    Language(crate::screens::language::LanguageScreen),
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
    /// A lazily fetched font subset for a script the default font lacks.
    FontBytes(&'static str, Result<Vec<u8>, String>),
    #[cfg(target_arch = "wasm32")]
    CameraReady(Result<crate::platform::web::camera::Camera, String>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HintMode {
    /// Today's behavior: star chips + every move shown.
    Full,
    /// Practice-stop: the guide halts at a chosen algorithm, shows only
    /// its NAME, and the user executes it from memory.
    Practice,
    /// Pure shortest solution, no algorithm weaving.
    Off,
}

pub struct HintSettings {
    pub mode: HintMode,
    /// Hard cap on extra moves vs the shortest solution (0..=12).
    pub max_extra: usize,
    /// Included algorithms in PRIORITY order (highest first).
    pub include: Vec<u16>,
}

impl Default for HintSettings {
    fn default() -> Self {
        HintSettings {
            mode: HintMode::Full,
            max_extra: 6,
            include: Vec::new(),
        }
    }
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
    /// Tap-selected face layer in Play mode (highlighted; turned by the
    /// on-screen arrows).
    pub selected_face: Option<cube_core::Face>,
    pub table: TableState,
    pub library: Library,
    /// Cases the user marks as "trained" (drives solver hints).
    pub trained: HashSet<u16>,
    /// Training attempts per case (persisted via cube-store in M6).
    pub attempts: HashMap<u16, Vec<Attempt>>,
    pub store: Option<cube_store::Store>,
    /// Standard light theme instead of the default dark (front-page
    /// toggle; persisted).
    pub light_mode: bool,
    /// Guided-solution preferences (gear settings; persisted).
    pub hints: HintSettings,
    /// Font subsets already requested (script name -> loaded).
    pub fonts_loaded: std::collections::HashSet<&'static str>,
    /// Tap-select mode: false = tap selects the SIDE you touched,
    /// true = CELL mode (center = that face, edge cell = the adjacent
    /// side it borders). Gear setting; persisted.
    pub tap_cell: bool,
    pub tx: Sender<AsyncMsg>,
    rx: Receiver<AsyncMsg>,
}

fn table_disabled() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        crate::platform::web::query_flag("notable")
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
}

fn store_disabled() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        crate::platform::web::query_flag("nostore")
    }
    #[cfg(not(target_arch = "wasm32"))]
    false
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

        #[cfg(target_arch = "wasm32")]
        crate::platform::web::crumb("new(): renderer+style ready");
        let table = if cfg!(target_arch = "wasm32") && table_disabled() {
            TableState::Failed("disabled via ?notable".into())
        } else {
            install_table_for_platform(&cc.egui_ctx, &tx)
        };
        #[cfg(target_arch = "wasm32")]
        crate::platform::web::crumb("new(): table fetch kicked off");
        let store = if cfg!(target_arch = "wasm32") && store_disabled() {
            None
        } else {
            crate::persist::open_store()
        };
        #[cfg(target_arch = "wasm32")]
        crate::platform::web::crumb("new(): store opened");

        let mut app = RubiksApp {
            screen: Screen::Menu,
            i18n: I18n { lang: Lang::EN },
            cube: FaceletCube::SOLVED,
            animator: MoveAnimator::default(),
            orbit: OrbitCamera::default(),
            rng: SplitMix64::new(seed | 1),
            history: Vec::new(),
            selected_face: None,
            table,
            library: Library::load(),
            trained: HashSet::new(),
            attempts: HashMap::new(),
            store,
            light_mode: false,
            hints: HintSettings::default(),
            fonts_loaded: std::collections::HashSet::new(),
            tap_cell: false,
            tx,
            rx,
        };
        crate::persist::warm_app(&mut app);
        app.light_mode = app
            .store
            .as_ref()
            .and_then(|s| s.setting("theme").ok().flatten())
            .is_some_and(|v| v == "light");
        apply_theme(&cc.egui_ctx, app.light_mode);
        // A remembered CJK language needs its font subset before the
        // first frame draws text.
        let lang = app.i18n.lang;
        app.ensure_font(lang, &cc.egui_ctx);
        #[cfg(target_arch = "wasm32")]
        crate::platform::web::crumb("new(): app warmed — startup complete");
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
        // Keep the guided-solution include list in sync: newly trained
        // algorithms join at the lowest priority; untrained ones leave.
        if trained {
            if !self.hints.include.contains(&case_idx) {
                self.hints.include.push(case_idx);
            }
        } else {
            self.hints.include.retain(|&c| c != case_idx);
        }
        self.save_hint_settings();
        if let Some(store) = &self.store {
            let id = self.library.rec.case(case_idx).id.clone();
            if let Err(e) = store.set_trained(&id, trained) {
                log::error!("set_trained: {e}");
            }
            crate::persist::persist(store);
        }
    }

    pub fn save_hint_settings(&self) {
        if let Some(store) = &self.store {
            let mode = match self.hints.mode {
                HintMode::Full => "full",
                HintMode::Practice => "practice",
                HintMode::Off => "off",
            };
            let ids: Vec<String> = self
                .hints
                .include
                .iter()
                .map(|&c| self.library.rec.case(c).id.clone())
                .collect();
            let _ = store.set_setting("hints.mode", mode);
            let _ = store.set_setting("hints.max_extra", &self.hints.max_extra.to_string());
            let _ = store.set_setting("hints.include", &ids.join(","));
            let _ = store.set_setting(
                "tap_select",
                if self.tap_cell { "cell" } else { "side" },
            );
            crate::persist::persist(store);
        }
    }

    pub fn set_lang(&mut self, lang: crate::i18n::Lang, ctx: &egui::Context) {
        self.i18n.lang = lang;
        self.ensure_font(lang, ctx);
        if let Some(store) = &self.store {
            if let Err(e) = store.set_setting("lang", lang.code()) {
                log::error!("set lang: {e}");
            }
            crate::persist::persist(store);
        }
    }

    /// Scripts the default font cannot draw (CJK) get a SUBSET font —
    /// only the characters this UI uses — fetched the first time such a
    /// language is chosen. Nothing ships in the bundle, so adding
    /// languages never grows the download for anyone else.
    pub fn ensure_font(&mut self, lang: crate::i18n::Lang, ctx: &egui::Context) {
        let Some(name) = lang.def().font else { return };
        if !self.fonts_loaded.insert(name) {
            return; // already loaded (or loading)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.tx.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = crate::platform::web::fetch_bytes(&format!("fonts/{name}.ttf")).await;
                let _ = tx.send(AsyncMsg::FontBytes(name, result));
                ctx.request_repaint();
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/");
            match std::fs::read(format!("{path}{name}.ttf")) {
                Ok(bytes) => install_font(ctx, name, bytes),
                Err(e) => log::warn!("font {name}: {e}"),
            }
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

    fn drain_async(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AsyncMsg::FontBytes(name, Ok(bytes)) => install_font(ctx, name, bytes),
                AsyncMsg::FontBytes(name, Err(e)) => log::warn!("font {name}: {e}"),
                AsyncMsg::TableBytes(Ok(bytes)) => {
                    // Packed asset (table.pack) since M7.
                    #[cfg(target_arch = "wasm32")]
                    crate::platform::web::crumb("table bytes received; decoding");
                    self.table = match cube_solver::install_packed_table(&bytes) {
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
        self.drain_async(ui.ctx());
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
                Screen::Settings(_) => screens::settings::show(self, ui),
                Screen::Language(_) => screens::language::show(self, ui),
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
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/table.pack"));
    match cube_solver::install_packed_table(TABLE_BYTES) {
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
        let result = crate::platform::web::fetch_bytes("table.pack").await;
        let _ = tx.send(AsyncMsg::TableBytes(result));
        ctx.request_repaint();
    });
    TableState::Loading
}

/// Install a fetched font as the LOWEST-priority fallback: the default
/// font keeps its shapes for Latin, and the subset only fills glyphs it
/// cannot draw.
pub fn install_font(ctx: &egui::Context, name: &'static str, bytes: Vec<u8>) {
    ctx.add_font(egui::epaint::text::FontInsert::new(
        name,
        egui::FontData::from_owned(bytes),
        vec![
            egui::epaint::text::InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: egui::epaint::text::FontPriority::Lowest,
            },
            egui::epaint::text::InsertFontFamily {
                family: egui::FontFamily::Monospace,
                priority: egui::epaint::text::FontPriority::Lowest,
            },
        ],
    ));
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

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::JsCast as _;

/// Dark (default) or standard light theme; toggled on the front page.
pub fn apply_theme(ctx: &egui::Context, light: bool) {
    ctx.all_styles_mut(|style| {
        let base = if light {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        style.visuals = base;
        style.visuals.panel_fill = if light {
            egui::Color32::from_rgb(0xF2, 0xF3, 0xF7)
        } else {
            egui::Color32::from_rgb(0x12, 0x15, 0x1D)
        };
    });
    // The PAGE behind the canvas must match too: on iOS the body peeks
    // out as a strip by the URL bar / home indicator — the spacing that
    // keeps buttons above the bar stays, but the strip must not show.
    #[cfg(target_arch = "wasm32")]
    {
        let color = if light { "#F2F3F7" } else { "#12151D" };
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            for el in [
                doc.document_element(),
                doc.body().map(web_sys::Element::from),
            ]
            .into_iter()
            .flatten()
            {
                if let Ok(html) = el.dyn_into::<web_sys::HtmlElement>() {
                    let _ = html.style().set_property("background", color);
                }
            }
        }
    }
}
