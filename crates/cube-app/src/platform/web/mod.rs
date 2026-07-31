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

/// Screen rotation angle (0/90/180/270) from the Screen Orientation API;
/// 0 when unavailable.
pub fn screen_angle() -> u16 {
    web_sys::window()
        .and_then(|w| w.screen().ok())
        .and_then(|s| s.orientation().angle().ok())
        .unwrap_or(0)
}
