#[cfg(feature = "cli")]
use crate::cli::Args;
use crate::cli::{Layout, MarginPreset};
use crate::model::{FontSettings, PbnMetadata};

use super::defaults::*;

/// Standard margin for bidding sheets (1/2 inch)
const BIDDING_SHEETS_MARGIN: f32 = 12.7;

/// Margins for declarer's plan layout
const DECLARERS_PLAN_MARGIN_LR: f32 = 12.7; // 1/2 inch left/right
const DECLARERS_PLAN_MARGIN_TB: f32 = 25.4; // 1 inch top/bottom

/// Runtime settings for PDF generation
#[derive(Debug, Clone)]
pub struct Settings {
    // Page dimensions
    pub page_width: f32,
    pub page_height: f32,
    pub margin: f32, // Single margin for backward compatibility
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub boards_per_page: u8,

    // CLI margin override (if specified)
    margin_preset: Option<MarginPreset>,

    // Layout style
    pub layout: Layout,

    // Display options
    pub show_bidding: bool,
    pub show_play: bool,
    pub show_commentary: bool,
    pub show_hcp: bool,
    pub justify: bool,
    pub debug_boxes: bool,
    /// Circle sure winners on declarer's plan layouts
    pub circle_sure_winners: bool,
    /// Circle promotable winners on declarer's plan layouts
    pub circle_promotable_winners: bool,
    /// Circle length winners on declarer's plan layouts
    pub circle_length_winners: bool,
    /// Multi-column layout mode (1 = single column, 2+ = multi-column)
    pub column_count: u8,
    /// Two-column auctions mode (show uncontested auctions in 2 columns)
    pub two_col_auctions: bool,
    /// Center layout mode (commentary first, board info centered below)
    pub center: bool,

    /// Title override from CLI (None = use metadata, Some("") = hide, Some(x) = use x)
    pub title_override: Option<String>,
    /// Title from metadata (HRTitleEvent)
    pub title_from_metadata: Option<String>,

    /// Board label format from %Translate directive
    /// Format string where "%" is replaced with the board number
    /// Default is "Board %" -> "Board 1", can be "%)" -> "1)"
    pub board_label_format: String,

    // Layout dimensions (in mm)
    pub hand_width: f32,
    pub hand_height: f32,
    pub compass_gap: f32,
    pub line_height: f32,

    // Typography (in points)
    pub title_font_size: f32,
    pub header_font_size: f32,
    pub body_font_size: f32,
    pub card_font_size: f32,
    pub compass_font_size: f32,
    pub commentary_font_size: f32,

    // Font specifications from PBN (for font family selection)
    pub fonts: FontSettings,

    // Bidding table
    pub bid_column_width: f32,
    pub bid_row_height: f32,

    // Colors (RGB 0.0-1.0)
    pub black_color: (f32, f32, f32),
    pub red_color: (f32, f32, f32),
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            page_width: 215.9, // Letter
            page_height: 279.4,
            margin: DEFAULT_PAGE_MARGIN,
            margin_top: DEFAULT_PAGE_MARGIN,
            margin_bottom: DEFAULT_PAGE_MARGIN,
            margin_left: DEFAULT_PAGE_MARGIN,
            margin_right: DEFAULT_PAGE_MARGIN,
            boards_per_page: 1,
            margin_preset: None,

            layout: Layout::Analysis,

            show_bidding: true,
            show_play: true,
            show_commentary: true,
            show_hcp: false,
            justify: false,
            debug_boxes: false,
            circle_sure_winners: false,
            circle_promotable_winners: false,
            circle_length_winners: false,
            column_count: 1,
            two_col_auctions: false,
            center: false,
            title_override: None,
            title_from_metadata: None,
            board_label_format: "Board %".to_string(),

            hand_width: DEFAULT_HAND_WIDTH,
            hand_height: DEFAULT_HAND_HEIGHT,
            compass_gap: DEFAULT_COMPASS_GAP,
            line_height: DEFAULT_LINE_HEIGHT,

            title_font_size: DEFAULT_TITLE_FONT_SIZE,
            header_font_size: DEFAULT_HEADER_FONT_SIZE,
            body_font_size: DEFAULT_BODY_FONT_SIZE,
            card_font_size: DEFAULT_CARD_FONT_SIZE,
            compass_font_size: 11.0,    // CardTable default
            commentary_font_size: 12.0, // Commentary default

            fonts: FontSettings::default(),

            bid_column_width: DEFAULT_BID_COLUMN_WIDTH,
            bid_row_height: DEFAULT_BID_ROW_HEIGHT,

            black_color: BLACK_SUIT_COLOR,
            red_color: RED_SUIT_COLOR,
        }
    }
}

impl Settings {
    /// Create settings from CLI arguments
    #[cfg(feature = "cli")]
    pub fn from_args(args: &Args) -> Self {
        let (page_width, page_height) = args.page_dimensions();

        // Determine initial margins based on layout and CLI override
        let (margin_lr, margin_tb) = if let Some(preset) = args.margins {
            let m = preset.size_mm();
            (m, m)
        } else if args.layout == Layout::BiddingSheets {
            // Bidding sheets use standard margins by default
            (BIDDING_SHEETS_MARGIN, BIDDING_SHEETS_MARGIN)
        } else if args.layout.is_declarers_plan() {
            // Declarer's plan uses 0.5" left/right, 1.0" top/bottom
            (DECLARERS_PLAN_MARGIN_LR, DECLARERS_PLAN_MARGIN_TB)
        } else {
            (DEFAULT_PAGE_MARGIN, DEFAULT_PAGE_MARGIN)
        };

        Self {
            page_width,
            page_height,
            margin: margin_lr,
            margin_top: margin_tb,
            margin_bottom: margin_tb,
            margin_left: margin_lr,
            margin_right: margin_lr,
            boards_per_page: args.boards_per_page,
            margin_preset: args.margins,
            layout: args.layout,
            show_bidding: args.show_bidding(),
            show_play: args.show_play(),
            show_commentary: args.show_commentary(),
            show_hcp: args.show_hcp(),
            debug_boxes: args.debug_boxes,
            circle_sure_winners: args.circle_sure_winners,
            circle_promotable_winners: args.circle_promotable_winners,
            circle_length_winners: args.circle_length_winners,
            title_override: args.title.clone(),
            ..Default::default()
        }
    }

    /// Create settings for a specific layout with appropriate defaults
    pub fn for_layout(layout: Layout) -> Self {
        let (margin_lr, margin_tb) = match layout {
            Layout::BiddingSheets => (BIDDING_SHEETS_MARGIN, BIDDING_SHEETS_MARGIN),
            Layout::DeclarersPlan | Layout::DeclarersPlan1up | Layout::DeclarersPlan2up => {
                (DECLARERS_PLAN_MARGIN_LR, DECLARERS_PLAN_MARGIN_TB)
            }
            Layout::DealerSummary => (DECLARERS_PLAN_MARGIN_LR, DECLARERS_PLAN_MARGIN_TB),
            Layout::Analysis => (DEFAULT_PAGE_MARGIN, DEFAULT_PAGE_MARGIN),
        };

        Self {
            margin: margin_lr,
            margin_top: margin_tb,
            margin_bottom: margin_tb,
            margin_left: margin_lr,
            margin_right: margin_lr,
            layout,
            ..Default::default()
        }
    }

    /// Merge with PBN metadata (embedded settings override defaults)
    pub fn with_metadata(mut self, metadata: &PbnMetadata) -> Self {
        if let Some(bpp) = metadata.layout.boards_per_page {
            self.boards_per_page = bpp;
        }

        // Apply PBN margins only if:
        // 1. No CLI margin override was specified, AND
        // 2. Layout is Analysis (bidding sheets and declarer's plan ignore embedded margins)
        if self.margin_preset.is_none() && self.layout == Layout::Analysis {
            if let Some(ref margins) = metadata.layout.margins {
                self.margin_top = margins.top;
                self.margin_bottom = margins.bottom;
                self.margin_left = margins.left;
                self.margin_right = margins.right;
                self.margin = margins.left; // Use left margin as general margin for backward compatibility
            }
        }

        // Apply font sizes from metadata
        self.card_font_size = metadata.fonts.diagram_size();
        self.body_font_size = metadata.fonts.hand_record_size();
        self.title_font_size = metadata.fonts.event_size();
        self.compass_font_size = metadata.fonts.card_table_size();
        self.commentary_font_size = metadata.fonts.commentary_size();

        // Apply colors from metadata
        let scale = |v: u8| v as f32 / 255.0;
        self.black_color = (
            scale(metadata.colors.spades.0),
            scale(metadata.colors.spades.1),
            scale(metadata.colors.spades.2),
        );
        self.red_color = (
            scale(metadata.colors.hearts.0),
            scale(metadata.colors.hearts.1),
            scale(metadata.colors.hearts.2),
        );

        // Store font settings for font family selection
        self.fonts = metadata.fonts.clone();

        // Apply display options from PBN metadata
        if metadata.layout.show_hcp {
            self.show_hcp = true;
        }
        if metadata.layout.justify {
            self.justify = true;
        }
        if metadata.layout.column_count > 1 {
            self.column_count = metadata.layout.column_count;
        }
        if metadata.layout.two_col_auctions {
            self.two_col_auctions = true;
        }
        if metadata.layout.center {
            self.center = true;
        }

        // Store title from metadata (HRTitleEvent)
        self.title_from_metadata = metadata.title_event.clone();

        // Apply board label format from %Translate directive
        if let Some(ref fmt) = metadata.layout.board_label_format {
            self.board_label_format = fmt.clone();
        }

        self
    }

    /// Get the effective title for display
    /// Returns None if title should be hidden, Some(title) otherwise
    pub fn effective_title(&self) -> Option<&str> {
        match &self.title_override {
            Some(t) if t.is_empty() => None, // --title with no value hides title
            Some(t) => Some(t.as_str()),     // --title "value" uses that value
            None => self.title_from_metadata.as_deref(), // Use metadata if no override
        }
    }

    /// Get the usable content area width
    pub fn content_width(&self) -> f32 {
        self.page_width - self.margin_left - self.margin_right
    }

    /// Get the usable content area height
    pub fn content_height(&self) -> f32 {
        self.page_height - self.margin_top - self.margin_bottom
    }

    /// Get the total width of the hand diagram (including compass)
    pub fn diagram_width(&self) -> f32 {
        (self.hand_width * 2.0) + self.compass_gap
    }

    /// Get the total height of the hand diagram (including compass)
    pub fn diagram_height(&self) -> f32 {
        (self.hand_height * 2.0) + self.compass_gap
    }
}
