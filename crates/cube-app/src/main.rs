#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cube_app::app::RubiksApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("Rubik's Cube"),
        ..Default::default()
    };
    eframe::run_native(
        "rubiks",
        options,
        Box::new(|cc| Ok(Box::new(RubiksApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("rubiks_canvas")
            .expect("canvas #rubiks_canvas missing in index.html")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#rubiks_canvas is not a canvas");
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(RubiksApp::new(cc)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
