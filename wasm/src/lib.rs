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

use pbn_to_pdf::cli::parse_board_range;
use pbn_to_pdf::{parse_pbn, render_boards, Board, Layout, RenderOptions};

/// Route panics to `console.error` with a real message and stack, instead of
/// the bare `unreachable executed` a wasm trap otherwise surfaces.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Card-circling flags for the declarer's plan layouts. Other layouts ignore
/// them. Construct with `new RenderOptions()` and set the fields you want.
#[wasm_bindgen(js_name = RenderOptions)]
#[derive(Debug, Default, Clone)]
pub struct WasmRenderOptions {
    /// Which boards to include, in the CLI's `--boards` syntax: `"1-8"`,
    /// `"5,8,12"`, or a mix. Empty or unset renders every board.
    ///
    /// Boards are selected by their `[Board]` number, not their position in the
    /// file, so a set numbered 17-32 will not respond to `"1"`. Use
    /// `renderFirstBoard` when you want the first board whatever it is called.
    #[wasm_bindgen(getter_with_clone, js_name = boards)]
    pub boards: Option<String>,
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

/// Apply an options `boards` spec, matching what the CLI's `--boards` does:
/// select by `[Board]` number, and drop boards that carry no number at all.
fn select_boards(boards: Vec<Board>, spec: Option<&str>) -> Result<Vec<Board>, JsError> {
    let Some(spec) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(boards);
    };
    let wanted =
        parse_board_range(spec).map_err(|e| JsError::new(&format!("Invalid board range: {e}")))?;
    let selected: Vec<Board> = boards
        .into_iter()
        .filter(|b| b.number.map(|n| wanted.contains(&n)).unwrap_or(false))
        .collect();
    if selected.is_empty() {
        // Distinct from an empty file: the boards exist, the spec missed them.
        return Err(JsError::new(&format!(
            "No boards matched '{spec}'. Boards are selected by their [Board] number, \
             which need not start at 1."
        )));
    }
    Ok(selected)
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
/// null, in which case every board is rendered and no cards are circled.
#[wasm_bindgen(js_name = renderPbn)]
pub fn render_pbn(
    pbn: &str,
    layout: &str,
    options: Option<WasmRenderOptions>,
) -> Result<Vec<u8>, JsError> {
    let options = options.unwrap_or_default();
    let spec = options.boards.clone();
    render_selected(pbn, layout, options, |boards| {
        select_boards(boards, spec.as_deref())
    })
}

/// Boards worth rendering to preview `layout` — see [`Layout::preview_boards`].
///
/// One for the 1-up plan, two for 2-up, four for 4-up, six for a dealer
/// summary, five for bidding sheets.
#[wasm_bindgen(js_name = previewBoardCount)]
pub fn preview_board_count(layout: &str) -> Result<u32, JsError> {
    Ok(Layout::from_str(layout)
        .map_err(|e| JsError::new(&e))?
        .preview_boards())
}

/// Render the first `count` boards in the file, whatever they are numbered.
///
/// This is how a preview is built: rendering a lesson's opening boards through
/// each layout costs a fraction of the whole set. Positional rather than by
/// `[Board]` number, because a sliced set may be numbered from anything and a
/// preview must not depend on that.
///
/// Pair it with [`preview_board_count`]. The result can run to more than one
/// page — bidding sheets in particular, where paging follows auction length —
/// and a preview is expected to show the first page and drop the rest.
#[wasm_bindgen(js_name = renderFirstBoards)]
pub fn render_first_boards(
    pbn: &str,
    layout: &str,
    count: usize,
    options: Option<WasmRenderOptions>,
) -> Result<Vec<u8>, JsError> {
    if count == 0 {
        return Err(JsError::new("count must be at least 1"));
    }
    render_selected(pbn, layout, options.unwrap_or_default(), |mut boards| {
        boards.truncate(count);
        Ok(boards)
    })
}

/// Render a preview of `layout`: its own [`preview_board_count`] of boards,
/// taken from the start of the file. Show the first page of the result.
#[wasm_bindgen(js_name = renderPreview)]
pub fn render_preview(
    pbn: &str,
    layout: &str,
    options: Option<WasmRenderOptions>,
) -> Result<Vec<u8>, JsError> {
    let count = preview_board_count(layout)? as usize;
    render_first_boards(pbn, layout, count, options)
}

/// The shared body of the render entry points: parse, choose boards, render.
fn render_selected(
    pbn: &str,
    layout: &str,
    options: WasmRenderOptions,
    choose: impl FnOnce(Vec<Board>) -> Result<Vec<Board>, JsError>,
) -> Result<Vec<u8>, JsError> {
    let layout = Layout::from_str(layout).map_err(|e| JsError::new(&e))?;
    let pbn_file = parse_pbn(pbn).map_err(|e| JsError::new(&e.to_string()))?;

    // parse_pbn accepts input with no recognisable boards and yields an empty
    // list; rendering that gives a blank PDF. Reject it here, as the CLI does,
    // so the caller gets an error rather than an empty document.
    if pbn_file.boards.is_empty() {
        return Err(JsError::new("No boards to process"));
    }
    let boards = choose(pbn_file.boards)?;

    // `%` header lines carry the Bridge Composer directives (title, options,
    // margins); render_boards parses them out of the raw comment lines.
    let metadata_comments: Vec<String> = pbn
        .lines()
        .filter(|line| line.starts_with('%'))
        .map(String::from)
        .collect();

    render_boards(&boards, &metadata_comments, layout, options.into())
        .map_err(|e| JsError::new(&e.to_string()))
}
