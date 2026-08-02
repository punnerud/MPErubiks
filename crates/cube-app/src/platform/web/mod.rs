//! Browser platform glue: fetch, and (from M4) camera access.

use eframe::wasm_bindgen::JsCast as _;

pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|e| format!("fetch failed: {e:?}"))?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "not a Response")?;
    if !resp.ok() {
        return Err(format!("HTTP {} for {url}", resp.status()));
    }
    let buf = wasm_bindgen_futures::JsFuture::from(
        resp.array_buffer().map_err(|e| format!("{e:?}"))?,
    )
    .await
    .map_err(|e| format!("read failed: {e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

pub mod camera;

/// Startup breadcrumb straight to the JS console — survives where the UI
/// can't render yet.
pub fn crumb(msg: &str) {
    web_sys::console::log_1(&format!("[rubiks] {msg}").into());
}

/// `?flag=1`-style kill-switches for bisecting startup crashes on devices
/// without a console.
pub fn query_flag(flag: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .map(|s| s.contains(flag))
        .unwrap_or(false)
}

/// Value of a `?name=value` query parameter, if present.
pub fn query_value(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == name).then(|| v.to_string())
        })
}

/// Screen rotation angle (0/90/180/270) from the Screen Orientation API;
/// 0 when unavailable.
pub fn screen_angle() -> u16 {
    web_sys::window()
        .and_then(|w| w.screen().ok())
        .and_then(|s| s.orientation().angle().ok())
        .unwrap_or(0)
}

/// Fire-and-forget JSON POST (scan-capture telemetry). Errors are logged,
/// never surfaced — data collection must not disturb the flow.
pub fn post_json_forget(url: &str, body: String) {
    let url = url.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_body(&eframe::wasm_bindgen::JsValue::from_str(&body));
        if let Some(window) = web_sys::window() {
            let _ = wasm_bindgen_futures::JsFuture::from(
                window.fetch_with_str_and_init(&url, &init),
            )
            .await;
        }
    });
}

pub fn hex_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for b in data {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}
