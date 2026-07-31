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
fn show_fatal(msg: &str) {
    let escaped = msg.replace('&', "&amp;").replace('<', "&lt;");
    if let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    {
        let _ = body.set_inner_html(&format!(
            "<div style=\"color:#fff;background:#8b1a1a;font:14px monospace;\
             padding:16px;white-space:pre-wrap\">Appen krasjet / app crashed:\n\n{escaped}</div>"
        ));
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Panics are invisible on phones (no console): paint them into the DOM.
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        web_sys::console::error_1(&msg.clone().into());
        show_fatal(&msg);
    }));

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
        if let Err(e) = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(RubiksApp::new(cc)))),
            )
            .await
        {
            show_fatal(&format!("eframe start failed: {e:?}"));
        }
    });
}
