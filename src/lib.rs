pub mod cli;
pub mod config;
pub mod embed;
pub mod error;
pub mod model;
pub mod parser;
pub mod render;

pub use cli::Layout;
pub use config::Settings;
pub use embed::{EmbedSource, Rendered};
pub use error::{PbnError, RenderError};
pub use model::Board;
pub use parser::{parse_pbn, PbnFile};
pub use render::generate_pdf;

use parser::header::parse_headers;
use render::helpers::BoardRegion;
use render::layouts::analysis::DocumentRenderer;
use render::{
    BiddingSheetsRenderer, DealerSummaryRenderer, DeclarersPlan1UpRenderer,
    DeclarersPlan2UpRenderer, DeclarersPlanRenderer, HandRecordRenderer,
};

/// Optional rendering flags passed through from library consumers.
///
/// Currently used by the declarer's plan layouts (1-up, 2-up, 4-up) to
/// highlight analysis-identified cards with colored circles. When multiple
/// analyses identify the same card the highest-priority color wins
/// (sure > promotable > length).
#[derive(Debug, Default, Clone, Copy)]
pub struct RenderOptions {
    /// Circle sure winners in red (priority 1, highest)
    pub circle_sure_winners: bool,
    /// Circle promotable winners in green (priority 2)
    pub circle_promotable_winners: bool,
    /// Circle length winners in blue (priority 3)
    pub circle_length_winners: bool,
    /// Draw commentary that stands inside an `[Auction]` or `[Play]` section.
    /// Bridge Composer discards it, so this is off unless a caller asks.
    pub section_commentary: bool,
    /// Leave out the page furniture the PBN asks for -- the event header or
    /// headings and the `%PageFooter` lines -- for a pipeline that adds its
    /// own. Off by default, so output matches Bridge Composer's.
    pub omit_page_furniture: bool,
}

/// High-level API for rendering boards to PDF.
///
/// This is the recommended entry point for library consumers. It handles all
/// rendering details internally - consumers just provide boards, metadata, and
/// layout choice.
///
/// # Arguments
///
/// * `boards` - Slice of parsed Board structs to render
/// * `metadata_comments` - Raw PBN header comments (lines starting with %)
///   like `%HRTitleEvent My Event`, `%BCOptions Justify ShowHCP`, etc.
/// * `layout` - The layout style to use for rendering
///
/// # Returns
///
/// PDF file contents as bytes, or a RenderError on failure.
///
/// # Example
///
/// ```no_run
/// use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};
///
/// let pbn_content = std::fs::read_to_string("hands.pbn").unwrap();
/// let pbn_file = parse_pbn(&pbn_content).unwrap();
///
/// // Extract metadata comments (lines starting with %)
/// let metadata_comments: Vec<String> = pbn_content
///     .lines()
///     .filter(|line| line.starts_with('%'))
///     .map(String::from)
///     .collect();
///
/// let pdf_bytes = render_boards(
///     &pbn_file.boards,
///     &metadata_comments,
///     Layout::DeclarersPlan,
///     RenderOptions::default(),
/// ).unwrap();
///
/// std::fs::write("output.pdf", pdf_bytes).unwrap();
/// ```
pub fn render_boards(
    boards: &[Board],
    metadata_comments: &[String],
    layout: Layout,
    options: RenderOptions,
) -> Result<Vec<u8>, RenderError> {
    render_with_settings(boards, &settings_for(metadata_comments, layout, options))
        .map(|(pdf, _)| pdf)
}

/// Render boards picked from a PBN file, with Contract 5's payload attached.
///
/// As [`render_boards`], plus the PBN profile of Contract 5 (contract in
/// lesson-stage, `docs/contracts/pdf-attachments.md`): the source file and the
/// rendered boards embedded in the PDF, a click map giving each board's page
/// and rectangle, and a `lesson-block:` link over each board. A presenter such
/// as lesson-stage can then tap a board and get its deal from the PDF alone.
///
/// `positions[i]` is the index of `boards[i]` in `parse_pbn(source.text)`, so
/// each embedded record is copied from the file as written.
pub fn render_boards_embedded(
    boards: &[Board],
    positions: &[usize],
    metadata_comments: &[String],
    layout: Layout,
    options: RenderOptions,
    source: &EmbedSource,
) -> Result<Rendered, RenderError> {
    let settings = settings_for(metadata_comments, layout, options);
    let (pdf, regions) = render_with_settings(boards, &settings)?;
    embed::embed(pdf, &regions, boards, positions, source, layout)
}

/// Settings with layout-appropriate defaults, the file's metadata, and the
/// caller's options.
fn settings_for(metadata_comments: &[String], layout: Layout, options: RenderOptions) -> Settings {
    // Parse metadata from raw comment lines
    let comment_refs: Vec<&str> = metadata_comments.iter().map(|s| s.as_str()).collect();
    let metadata = parse_headers(&comment_refs);

    // Create settings with layout-appropriate defaults, then apply metadata
    let mut settings = Settings::for_layout(layout).with_metadata(&metadata);
    settings.circle_sure_winners = options.circle_sure_winners;
    settings.circle_promotable_winners = options.circle_promotable_winners;
    settings.circle_length_winners = options.circle_length_winners;
    settings.section_commentary = options.section_commentary;
    settings.page_furniture = !options.omit_page_furniture;
    settings
}

/// Render with fully-built settings, in `settings.layout`, and report where each
/// board was drawn. The CLI comes in here, since its settings come from its
/// arguments rather than [`RenderOptions`].
pub fn render_with_settings(
    boards: &[Board],
    settings: &Settings,
) -> Result<(Vec<u8>, Vec<BoardRegion>), RenderError> {
    let settings = settings.clone();
    match settings.layout {
        Layout::Analysis => DocumentRenderer::new(settings).render_with_regions(boards),
        Layout::BiddingSheets => BiddingSheetsRenderer::new(settings).render_with_regions(boards),
        Layout::DeclarersPlan1up => {
            DeclarersPlan1UpRenderer::new(settings).render_with_regions(boards)
        }
        Layout::DeclarersPlan2up => {
            DeclarersPlan2UpRenderer::new(settings).render_with_regions(boards)
        }
        Layout::DeclarersPlan => DeclarersPlanRenderer::new(settings).render_with_regions(boards),
        Layout::DealerSummary => DealerSummaryRenderer::new(settings).render_with_regions(boards),
        Layout::HandRecord => HandRecordRenderer::new(settings).render_with_regions(boards),
    }
}
