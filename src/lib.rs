#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub use app::HexGameUi;

// ----------------------------------------------------------------------------
// When compiling for web:

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::{self, prelude::*};

/// This is the entry-point for all the web-assembly.
/// This is called once from the HTML.
/// It loads the app, installs some callbacks, then returns.
/// You can add more callbacks like this if you want to call in to your code.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    let app = HexGameUi::default();
    eframe::start_web(canvas_id, Box::new(app))
}
