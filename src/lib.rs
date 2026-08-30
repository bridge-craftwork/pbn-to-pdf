pub mod cli;
pub mod config;
pub mod error;
pub mod model;
pub mod parser;
pub mod render;

pub use cli::Layout;
pub use config::Settings;
pub use error::{PbnError, RenderError};
pub use model::Board;
pub use parser::{parse_pbn, PbnFile};
pub use render::generate_pdf;

use parser::header::parse_headers;
use render::{
    BiddingSheetsRenderer, DealerSummaryRenderer, DeclarersPlan1UpRenderer,
    DeclarersPlan2UpRenderer, DeclarersPlanRenderer,
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
    // Parse metadata from raw comment lines
    let comment_refs: Vec<&str> = metadata_comments.iter().map(|s| s.as_str()).collect();
    let metadata = parse_headers(&comment_refs);

    // Create settings with layout-appropriate defaults, then apply metadata
    let mut settings = Settings::for_layout(layout).with_metadata(&metadata);
    settings.circle_sure_winners = options.circle_sure_winners;
    settings.circle_promotable_winners = options.circle_promotable_winners;
    settings.circle_length_winners = options.circle_length_winners;

    // Route to the appropriate renderer based on layout
    match layout {
        Layout::Analysis => generate_pdf(boards, &settings),
        Layout::BiddingSheets => {
            let renderer = BiddingSheetsRenderer::new(settings);
            renderer.render(boards)
        }
        Layout::DeclarersPlan1up => {
            let renderer = DeclarersPlan1UpRenderer::new(settings);
            renderer.render(boards)
        }
        Layout::DeclarersPlan2up => {
            let renderer = DeclarersPlan2UpRenderer::new(settings);
            renderer.render(boards)
        }
        Layout::DeclarersPlan => {
            let renderer = DeclarersPlanRenderer::new(settings);
            renderer.render(boards)
        }
        Layout::DealerSummary => {
            let renderer = DealerSummaryRenderer::new(settings);
            renderer.render(boards)
        }
    }
}
