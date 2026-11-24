//! WebAssembly bindings.
//!
//! A thin wrapper over [`pbn_to_pdf::render_boards_embedded`], which is already a pure
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
use pbn_to_pdf::{
    parse_pbn, render_boards, render_boards_embedded, Board, EmbedSource, Layout, RenderOptions,
};

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
    /// Draw the play record of every board that has one (analysis layout),
    /// not only those whose `BCFlags` ask for it.
    #[wasm_bindgen(js_name = allPlayRecords)]
    pub all_play_records: bool,
    /// Lay the play record out by seat, with double-dummy costs.
    #[wasm_bindgen(js_name = playBySeat)]
    pub play_by_seat: bool,
    /// Leave out the page furniture -- event header or headings and the
    /// `%PageFooter` lines. Off by default, matching Bridge Composer.
    #[wasm_bindgen(js_name = omitPageFurniture)]
    pub omit_page_furniture: bool,
    /// Leave out the Contract 5 payload: the source PBN, a click map of where
    /// each board is, and a `lesson-block:` link over each board. Embedded by
    /// default; a preview that is only ever shown can skip it.
    #[wasm_bindgen(js_name = noEmbed)]
    pub no_embed: bool,
    /// The filename the embedded source carries. Defaults to `source.pbn`.
    #[wasm_bindgen(getter_with_clone, js_name = sourceName)]
    pub source_name: Option<String>,
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
            // Commentary written inside an [Auction] or [Play] section is
            // section data, which BridgeComposer leaves out. The browser has no
            // reason to ask for it, so it takes the matching default.
            section_commentary: false,
            all_play_records: options.all_play_records,
            play_by_seat: options.play_by_seat,
            omit_page_furniture: options.omit_page_furniture,
        }
    }
}

/// Apply an options `boards` spec, matching what the CLI's `--boards` does:
/// select by `[Board]` number, and drop boards that carry no number at all.
fn select_boards(
    boards: Vec<(usize, Board)>,
    spec: Option<&str>,
) -> Result<Vec<(usize, Board)>, JsError> {
    let Some(spec) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(boards);
    };
    let wanted =
        parse_board_range(spec).map_err(|e| JsError::new(&format!("Invalid board range: {e}")))?;
    let selected: Vec<(usize, Board)> = boards
        .into_iter()
        .filter(|(_, b)| b.number.map(|n| wanted.contains(&n)).unwrap_or(false))
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
    .map(|(pdf, _)| pdf)
}

/// A PDF with its Contract 5 click map: each board's page and rectangle.
#[wasm_bindgen(js_name = RenderedPdf, getter_with_clone)]
pub struct WasmRendered {
    pub pdf: Vec<u8>,
    /// `lesson-blocks.json` as embedded; empty when `noEmbed` was set.
    #[wasm_bindgen(js_name = clickMap)]
    pub click_map: String,
}

/// As [`render_pbn`], with the click map handed back alongside the PDF, so a
/// page can tell which page each board landed on.
#[wasm_bindgen(js_name = renderPbnWithMap)]
pub fn render_pbn_with_map(
    pbn: &str,
    layout: &str,
    options: Option<WasmRenderOptions>,
) -> Result<WasmRendered, JsError> {
    let options = options.unwrap_or_default();
    let spec = options.boards.clone();
    let (pdf, click_map) = render_selected(pbn, layout, options, |boards| {
        select_boards(boards, spec.as_deref())
    })?;
    Ok(WasmRendered {
        pdf,
        click_map: click_map.unwrap_or_default(),
    })
}

/// Where each board's record sits in `pbn`, as a flat array of UTF-16 offsets
/// `[start0, end0, start1, end1, ...]` -- JavaScript string indices. Record
/// `i` is board `i + 1` of a full render's click map. Empty if the text won't
/// read as PBN.
#[wasm_bindgen(js_name = sourceRecords)]
pub fn source_records(pbn: &str) -> Vec<u32> {
    let Some(spans) = pbn_to_pdf::source_records(pbn) else {
        return Vec::new();
    };
    // Byte offsets to UTF-16 ones, walking the text once.
    let mut out = Vec::with_capacity(spans.len() * 2);
    let mut wanted: Vec<usize> = spans.iter().flat_map(|&(a, b)| [a, b]).collect();
    wanted.reverse();
    let mut units = 0u32;
    for (byte, ch) in pbn.char_indices().chain(std::iter::once((pbn.len(), ' '))) {
        while wanted.last() == Some(&byte) {
            out.push(units);
            wanted.pop();
        }
        units += ch.len_utf16() as u32;
    }
    out
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
    .map(|(pdf, _)| pdf)
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
/// The click map comes back with an embedded render.
fn render_selected(
    pbn: &str,
    layout: &str,
    options: WasmRenderOptions,
    choose: impl FnOnce(Vec<(usize, Board)>) -> Result<Vec<(usize, Board)>, JsError>,
) -> Result<(Vec<u8>, Option<String>), JsError> {
    let layout = Layout::from_str(layout).map_err(|e| JsError::new(&e))?;
    let pbn_file = parse_pbn(pbn).map_err(|e| JsError::new(&e.to_string()))?;

    // parse_pbn accepts input with no recognisable boards and yields an empty
    // list; rendering that gives a blank PDF. Reject it here, as the CLI does,
    // so the caller gets an error rather than an empty document.
    if pbn_file.boards.is_empty() {
        return Err(JsError::new("No boards to process"));
    }
    // Each board keeps its place in the file, so the embedded records can be
    // copied from the source as written.
    let (positions, boards): (Vec<usize>, Vec<Board>) =
        choose(pbn_file.boards.into_iter().enumerate().collect())?
            .into_iter()
            .unzip();

    // `%` header lines carry the Bridge Composer directives (title, options,
    // margins); render_boards parses them out of the raw comment lines.
    let metadata_comments: Vec<String> = pbn
        .lines()
        .filter(|line| line.starts_with('%'))
        .map(String::from)
        .collect();

    if options.no_embed {
        return render_boards(&boards, &metadata_comments, layout, options.into())
            .map(|pdf| (pdf, None))
            .map_err(|e| JsError::new(&e.to_string()));
    }
    let name = options
        .source_name
        .clone()
        .unwrap_or_else(|| "source.pbn".to_string());
    let source = EmbedSource {
        name: &name,
        text: pbn,
    };
    render_boards_embedded(
        &boards,
        &positions,
        &metadata_comments,
        layout,
        options.into(),
        &source,
    )
    .map(|rendered| (rendered.pdf, Some(rendered.click_map)))
    .map_err(|e| JsError::new(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_records_are_in_utf16_units() {
        // "♠" is three bytes and one UTF-16 unit; "🂡" four bytes and two.
        let pbn = "{♠🂡}\n[Board \"1\"]\n[Deal \"N:AKQJ.T98.765.432 - - -\"]\n";
        let spans = source_records(pbn);
        let utf16: Vec<u16> = pbn.encode_utf16().collect();
        let record = String::from_utf16(&utf16[spans[0] as usize..spans[1] as usize]).unwrap();
        assert!(record.contains("[Board \"1\"]"), "{record:?}");
    }
}
