//! WebAssembly bindings.
//!
//! A thin wrapper over [`pbn_to_pdf::render_boards`], which is already a pure
//! `&[Board] -> Vec<u8>` function: no filesystem, no clock, no environment.
//! Fonts and the 52 card SVGs are compiled in, so a browser or Node host needs
//! nothing but the PBN text.
//!
//! ```js
//! import init, { renderPbn, layouts } from './pkg/pbn_to_pdf_wasm.js';
//! await init();
//! const pdf = renderPbn(pbnText, 'declarers-plan-2up');  // Uint8Array
//! ```

use std::str::FromStr;

use wasm_bindgen::prelude::*;

use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};

/// Route panics to `console.error` with a real message and stack, instead of
/// the bare `unreachable executed` a wasm trap otherwise surfaces.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Card-circling flags for the declarer's plan layouts. Other layouts ignore
/// them. Construct with `new RenderOptions()` and set the fields you want.
#[wasm_bindgen(js_name = RenderOptions)]
#[derive(Debug, Default, Clone, Copy)]
pub struct WasmRenderOptions {
    /// Circle sure winners in red (highest priority).
    #[wasm_bindgen(js_name = circleSureWinners)]
    pub circle_sure_winners: bool,
    /// Circle promotable winners in green.
    #[wasm_bindgen(js_name = circlePromotableWinners)]
    pub circle_promotable_winners: bool,
    /// Circle length winners in blue.
    #[wasm_bindgen(js_name = circleLengthWinners)]
    pub circle_length_winners: bool,
}

#[wasm_bindgen(js_class = RenderOptions)]
impl WasmRenderOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<WasmRenderOptions> for RenderOptions {
    fn from(options: WasmRenderOptions) -> Self {
        RenderOptions {
            circle_sure_winners: options.circle_sure_winners,
            circle_promotable_winners: options.circle_promotable_winners,
            circle_length_winners: options.circle_length_winners,
        }
    }
}

/// The layout names `renderPbn` accepts, in CLI order.
#[wasm_bindgen]
pub fn layouts() -> Vec<String> {
    Layout::ALL
        .iter()
        .map(|layout| layout.as_str().to_string())
        .collect()
}

/// Number of boards in a PBN document, without rendering it.
#[wasm_bindgen(js_name = boardCount)]
pub fn board_count(pbn: &str) -> Result<usize, JsError> {
    let pbn_file = parse_pbn(pbn).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(pbn_file.boards.len())
}

/// Render PBN text to PDF bytes.
///
/// `layout` is one of the names from [`layouts`]. `options` may be omitted or
/// null, in which case no cards are circled.
#[wasm_bindgen(js_name = renderPbn)]
pub fn render_pbn(
    pbn: &str,
    layout: &str,
    options: Option<WasmRenderOptions>,
) -> Result<Vec<u8>, JsError> {
    let layout = Layout::from_str(layout).map_err(|e| JsError::new(&e))?;
    let pbn_file = parse_pbn(pbn).map_err(|e| JsError::new(&e.to_string()))?;

    // parse_pbn accepts input with no recognisable boards and yields an empty
    // list; rendering that gives a blank PDF. Reject it here, as the CLI does,
    // so the caller gets an error rather than an empty document.
    if pbn_file.boards.is_empty() {
        return Err(JsError::new("No boards to process"));
    }

    // `%` header lines carry the Bridge Composer directives (title, options,
    // margins); render_boards parses them out of the raw comment lines.
    let metadata_comments: Vec<String> = pbn
        .lines()
        .filter(|line| line.starts_with('%'))
        .map(String::from)
        .collect();

    render_boards(
        &pbn_file.boards,
        &metadata_comments,
        layout,
        options.unwrap_or_default().into(),
    )
    .map_err(|e| JsError::new(&e.to_string()))
}
