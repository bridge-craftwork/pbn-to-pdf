#[cfg(feature = "cli")]
use clap::Parser;
#[cfg(feature = "cli")]
use std::path::PathBuf;

#[cfg(feature = "cli")]
#[derive(Parser, Debug)]
#[command(name = "pbn-to-pdf")]
#[command(
    author,
    version,
    about = "Convert PBN bridge files to PDF with Bridge Composer-style formatting"
)]
pub struct Args {
    /// Input PBN file path
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output PDF file path (defaults to input with .pdf extension)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Number of boards per page (1, 2, or 4)
    #[arg(short = 'n', long, default_value = "1", value_parser = clap::value_parser!(u8).range(1..=4))]
    pub boards_per_page: u8,

    /// Page size
    #[arg(short = 's', long, value_enum, default_value = "letter")]
    pub page_size: PageSize,

    /// Page orientation
    #[arg(long, value_enum, default_value = "portrait")]
    pub orientation: Orientation,

    /// Output layout style
    #[arg(short = 'l', long, value_enum, default_value = "analysis")]
    pub layout: Layout,

    /// Hide bidding table
    #[arg(long)]
    pub no_bidding: bool,

    /// Hide play sequence
    #[arg(long)]
    pub no_play: bool,

    /// Hide commentary text
    #[arg(long)]
    pub no_commentary: bool,

    /// Hide HCP point counts
    #[arg(long)]
    pub no_hcp: bool,

    /// Board range to include (e.g., "1-16" or "5,8,12")
    #[arg(short = 'b', long)]
    pub boards: Option<String>,

    /// Page margins (overrides PBN %Margins)
    #[arg(short = 'm', long, value_enum)]
    pub margins: Option<MarginPreset>,

    /// Draw debug boxes around layout regions
    #[arg(long)]
    pub debug_boxes: bool,

    /// Circle sure winners on cards (declarer's plan layouts) - red
    #[arg(long)]
    pub circle_sure_winners: bool,

    /// Circle promotable winners on cards (declarer's plan layouts) - green
    #[arg(long)]
    pub circle_promotable_winners: bool,

    /// Circle length winners on cards (declarer's plan layouts) - blue
    #[arg(long)]
    pub circle_length_winners: bool,

    /// Title for bidding sheets banner. Overrides %HRTitleEvent.
    /// Use --title with no value to hide the title.
    #[arg(short = 't', long, num_args = 0..=1, default_missing_value = "")]
    pub title: Option<String>,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Preset margin sizes
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum MarginPreset {
    /// Narrow margins (1/4 inch = 6.35mm)
    Narrow,
    /// Standard margins (1/2 inch = 12.7mm)
    Standard,
    /// Wide margins (1 inch = 25.4mm)
    Wide,
}

impl MarginPreset {
    /// Get the margin size in mm
    pub fn size_mm(&self) -> f32 {
        match self {
            MarginPreset::Narrow => 6.35,   // 1/4 inch
            MarginPreset::Standard => 12.7, // 1/2 inch
            MarginPreset::Wide => 25.4,     // 1 inch
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum PageSize {
    Letter,
    A4,
    Legal,
}

impl PageSize {
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PageSize::Letter => (215.9, 279.4),
            PageSize::A4 => (210.0, 297.0),
            PageSize::Legal => (215.9, 355.6),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// Output layout style
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Layout {
    /// Standard analysis layout with hand diagram, bidding, and commentary
    #[default]
    Analysis,
    /// Bidding practice sheets for face-to-face practice
    BiddingSheets,
    /// Declarer's plan - 1 deal per page (full size)
    #[cfg_attr(feature = "cli", value(name = "declarers-plan-1up"))]
    DeclarersPlan1up,
    /// Declarer's plan - 2 deals per page (rotated 90°)
    #[cfg_attr(feature = "cli", value(name = "declarers-plan-2up"))]
    DeclarersPlan2up,
    /// Declarer's plan practice sheets (4 deals per page)
    DeclarersPlan,
    /// Dealer summary showing board, dealer, contract, declarer, and lead (6 per page)
    DealerSummary,
}

impl Layout {
    /// Get the suffix to append to output filename (without extension)
    pub fn output_suffix(&self) -> Option<&'static str> {
        match self {
            Layout::Analysis => None,
            Layout::BiddingSheets => Some(" - Bidding Sheets"),
            Layout::DeclarersPlan1up => Some(" - Declarers Plan"),
            Layout::DeclarersPlan2up => Some(" - Declarers Plan 2up"),
            Layout::DeclarersPlan => Some(" - Declarers Plan 4up"),
            Layout::DealerSummary => Some(" - Dealer Summary"),
        }
    }

    /// Returns true if this is any declarer's plan variant
    pub fn is_declarers_plan(&self) -> bool {
        matches!(
            self,
            Layout::DeclarersPlan | Layout::DeclarersPlan1up | Layout::DeclarersPlan2up
        )
    }

    /// Boards to render for a preview of this layout.
    ///
    /// Enough to make the first page representative: fewer leaves it looking
    /// emptier than the layout really is. The consumer shows the first page and
    /// discards any others.
    ///
    /// Most layouts have a fixed geometry, so this is exactly what fits. Bidding
    /// sheets do not — how many boards land on a page depends on how long the
    /// auctions are — so five is a good sample rather than a capacity, and the
    /// extra pages it may produce are simply not shown.
    ///
    /// `preview_boards_renders_a_representative_first_page` in the integration
    /// tests holds these to their meaning by rendering them.
    pub fn preview_boards(&self) -> u32 {
        match self {
            Layout::Analysis => 1,
            Layout::BiddingSheets => 5,
            Layout::DeclarersPlan1up => 1,
            Layout::DeclarersPlan2up => 2,
            Layout::DeclarersPlan => 4,
            Layout::DealerSummary => 6,
        }
    }

    /// Every layout, in CLI declaration order.
    pub const ALL: [Layout; 6] = [
        Layout::Analysis,
        Layout::BiddingSheets,
        Layout::DeclarersPlan1up,
        Layout::DeclarersPlan2up,
        Layout::DeclarersPlan,
        Layout::DealerSummary,
    ];

    /// The layout's canonical name, identical to its `--layout` spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Layout::Analysis => "analysis",
            Layout::BiddingSheets => "bidding-sheets",
            Layout::DeclarersPlan1up => "declarers-plan-1up",
            Layout::DeclarersPlan2up => "declarers-plan-2up",
            Layout::DeclarersPlan => "declarers-plan",
            Layout::DealerSummary => "dealer-summary",
        }
    }
}

impl std::fmt::Display for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Layout {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Layout::ALL
            .iter()
            .copied()
            .find(|layout| layout.as_str() == s)
            .ok_or_else(|| {
                let names: Vec<&str> = Layout::ALL.iter().map(|l| l.as_str()).collect();
                format!(
                    "unknown layout '{}' (expected one of: {})",
                    s,
                    names.join(", ")
                )
            })
    }
}

#[cfg(feature = "cli")]
impl Args {
    /// Get the output path, defaulting to input with layout-specific suffix
    pub fn output_path(&self) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            // Get the input file stem (name without extension)
            let stem = self.input.file_stem().unwrap_or_default().to_string_lossy();

            // Add layout-specific suffix if applicable
            let new_name = if let Some(suffix) = self.layout.output_suffix() {
                format!("{}{}.pdf", stem, suffix)
            } else {
                format!("{}.pdf", stem)
            };

            // Keep the same directory as the input file
            if let Some(parent) = self.input.parent() {
                parent.join(new_name)
            } else {
                PathBuf::from(new_name)
            }
        })
    }

    /// Get page dimensions in mm (width, height) accounting for orientation
    pub fn page_dimensions(&self) -> (f32, f32) {
        let (w, h) = self.page_size.dimensions_mm();
        match self.orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }

    /// Check if bidding should be shown
    pub fn show_bidding(&self) -> bool {
        !self.no_bidding
    }

    /// Check if play should be shown
    pub fn show_play(&self) -> bool {
        !self.no_play
    }

    /// Check if commentary should be shown
    pub fn show_commentary(&self) -> bool {
        !self.no_commentary
    }

    /// Check if HCP should be shown
    pub fn show_hcp(&self) -> bool {
        !self.no_hcp
    }
}

/// Parse a board range specification
pub fn parse_board_range(spec: &str) -> Result<Vec<u32>, String> {
    let mut boards = Vec::new();

    for part in spec.split(',') {
        let part = part.trim();

        if part.contains('-') {
            // Range: "1-16"
            let parts: Vec<&str> = part.split('-').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid range: {}", part));
            }

            let start: u32 = parts[0]
                .trim()
                .parse()
                .map_err(|_| format!("Invalid number: {}", parts[0]))?;
            let end: u32 = parts[1]
                .trim()
                .parse()
                .map_err(|_| format!("Invalid number: {}", parts[1]))?;

            if start > end {
                return Err(format!("Invalid range: {} > {}", start, end));
            }

            for i in start..=end {
                boards.push(i);
            }
        } else {
            // Single number
            let num: u32 = part
                .parse()
                .map_err(|_| format!("Invalid number: {}", part))?;
            boards.push(num);
        }
    }

    Ok(boards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_names_round_trip() {
        for layout in Layout::ALL {
            assert_eq!(layout.as_str().parse::<Layout>(), Ok(layout));
        }
    }

    /// `as_str` is what non-clap consumers (wasm) parse; clap's ValueEnum names
    /// are what the CLI accepts. They must stay identical.
    #[cfg(feature = "cli")]
    #[test]
    fn layout_names_match_clap_value_names() {
        use clap::ValueEnum;
        for layout in Layout::ALL {
            let clap_name = layout.to_possible_value().unwrap();
            assert_eq!(clap_name.get_name(), layout.as_str());
        }
    }

    #[test]
    fn test_parse_single_board() {
        let result = parse_board_range("5").unwrap();
        assert_eq!(result, vec![5]);
    }

    #[test]
    fn test_parse_range() {
        let result = parse_board_range("1-4").unwrap();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_mixed() {
        let result = parse_board_range("1-3, 7, 10-12").unwrap();
        assert_eq!(result, vec![1, 2, 3, 7, 10, 11, 12]);
    }

    #[cfg(feature = "cli")]
    #[test]
    fn test_page_dimensions() {
        let args = Args {
            input: PathBuf::from("test.pbn"),
            output: None,
            boards_per_page: 1,
            page_size: PageSize::Letter,
            orientation: Orientation::Portrait,
            layout: Layout::Analysis,
            no_bidding: false,
            no_play: false,
            no_commentary: false,
            no_hcp: false,
            boards: None,
            margins: None,
            debug_boxes: false,
            circle_sure_winners: false,
            circle_promotable_winners: false,
            circle_length_winners: false,
            title: None,
            verbose: 0,
        };

        let (w, h) = args.page_dimensions();
        assert!((w - 215.9).abs() < 0.1);
        assert!((h - 279.4).abs() < 0.1);
    }
}
