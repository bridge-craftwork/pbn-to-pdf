use crate::model::metadata::{ColorSettings, FontSpec, Margins, PaperSize, PbnMetadata};

/// Parse a PBN header line starting with %
pub fn parse_header_line(line: &str) -> Option<HeaderDirective> {
    let line = line.trim();

    if !line.starts_with('%') {
        return None;
    }

    let content = &line[1..].trim();

    // Parse different header types
    if let Some(stripped) = content.strip_prefix("PBN ") {
        let version = stripped.trim().to_string();
        return Some(HeaderDirective::Version(version));
    }

    if let Some(stripped) = content.strip_prefix("Creator:") {
        let creator = stripped.trim().to_string();
        return Some(HeaderDirective::Creator(creator));
    }

    if let Some(stripped) = content.strip_prefix("Created:") {
        let created = stripped.trim().to_string();
        return Some(HeaderDirective::Created(created));
    }

    if let Some(stripped) = content.strip_prefix("BoardsPerPage ") {
        let value = stripped.trim();
        if let Some(num) = parse_boards_per_page(value) {
            return Some(HeaderDirective::BoardsPerPage(num));
        }
    }

    if let Some(stripped) = content.strip_prefix("Margins ") {
        let value = stripped.trim();
        if let Some(margins) = parse_margins(value) {
            return Some(HeaderDirective::Margins(margins));
        }
    }

    if let Some(stripped) = content.strip_prefix("PaperSize ") {
        let value = stripped.trim();
        if let Some(size) = parse_paper_size(value) {
            return Some(HeaderDirective::PaperSize(size));
        }
    }

    if content.starts_with("Font:") {
        if let Some((name, settings)) = parse_font_directive(content) {
            return Some(HeaderDirective::Font(name, settings));
        }
    }

    if let Some(stripped) = content.strip_prefix("PipColors ") {
        let value = stripped.trim();
        if let Some(colors) = parse_pip_colors(value) {
            return Some(HeaderDirective::PipColors(colors));
        }
    }

    if let Some(stripped) = content.strip_prefix("HRTitleEvent ") {
        let value = stripped.trim();
        let title = value.trim_matches('"').to_string();
        return Some(HeaderDirective::TitleEvent(title));
    }

    if let Some(stripped) = content.strip_prefix("HRTitleDate ") {
        let value = stripped.trim();
        let date = value.trim_matches('"').to_string();
        return Some(HeaderDirective::TitleDate(date));
    }

    if content.starts_with("ShowHCP") {
        return Some(HeaderDirective::ShowHcp(true));
    }

    // Parse BCOptions line: "%BCOptions Float Justify NoHRStats STBorder STShade ShowHCP"
    if let Some(stripped) = content.strip_prefix("BCOptions ") {
        return Some(HeaderDirective::BCOptions(parse_bc_options(stripped)));
    }

    if let Some(stripped) = content.strip_prefix("ShowCardTable ") {
        let value = stripped.trim();
        let show = value != "0";
        return Some(HeaderDirective::ShowCardTable(show));
    }

    if let Some(stripped) = content.strip_prefix("ShowBoardLabels ") {
        let value = stripped.trim();
        let show = value != "0";
        return Some(HeaderDirective::ShowBoardLabels(show));
    }

    // Parse %Translate directive: %Translate "Board %" "%)"
    // This defines how board labels should be formatted
    if let Some(stripped) = content.strip_prefix("Translate ") {
        if let Some((from, to)) = parse_translate(stripped) {
            // Only handle "Board %" translations for now
            if from == "Board %" {
                return Some(HeaderDirective::BoardLabelFormat(to));
            }
        }
    }

    // Unknown directive
    Some(HeaderDirective::Unknown(content.to_string()))
}

/// Options parsed from %BCOptions line
#[derive(Debug, Clone, Default)]
pub struct BCOptions {
    pub justify: bool,
    pub show_hcp: bool,
    pub float: bool,
    pub center: bool,
    pub two_col_auctions: bool,
}

#[derive(Debug, Clone)]
pub enum HeaderDirective {
    Version(String),
    Creator(String),
    Created(String),
    BoardsPerPage(BoardsPerPageConfig),
    Margins(Margins),
    PaperSize(PaperSize),
    Font(String, FontSpec),
    PipColors(ColorSettings),
    TitleEvent(String),
    TitleDate(String),
    ShowHcp(bool),
    ShowCardTable(bool),
    ShowBoardLabels(bool),
    BCOptions(BCOptions),
    /// Board label format from %Translate "Board %" "%)"
    BoardLabelFormat(String),
    Unknown(String),
}

/// Parsed BoardsPerPage directive
#[derive(Debug, Clone, Copy)]
pub struct BoardsPerPageConfig {
    pub count: u8,
    pub fit: bool,
}

fn parse_boards_per_page(value: &str) -> Option<BoardsPerPageConfig> {
    // Format: "fit,1" or "fit,2" or just "1"
    let parts: Vec<&str> = value.split(',').collect();

    let (fit, num_str) = if parts.len() >= 2 && parts[0].trim().eq_ignore_ascii_case("fit") {
        (true, parts[1])
    } else {
        (false, *parts.last()?)
    };

    let count: u8 = num_str.trim().parse().ok()?;
    Some(BoardsPerPageConfig { count, fit })
}

fn parse_margins(value: &str) -> Option<Margins> {
    // Format: "1000,1000,500,750" (top, bottom, right, left in twips, where 1000 twips = 1 inch)
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 4 {
        return None;
    }

    // Convert from twips to mm (1000 twips = 1 inch = 25.4mm)
    let scale = 25.4 / 1000.0;

    Some(Margins {
        top: parts[0].trim().parse::<f32>().ok()? * scale,
        bottom: parts[1].trim().parse::<f32>().ok()? * scale,
        right: parts[2].trim().parse::<f32>().ok()? * scale,
        left: parts[3].trim().parse::<f32>().ok()? * scale,
    })
}

fn parse_paper_size(value: &str) -> Option<PaperSize> {
    // Format: "1,2159,2794,2" where 2159x2794 is dimensions in 1/100 mm
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() >= 3 {
        let width: f32 = parts[1].trim().parse().ok()?;
        let height: f32 = parts[2].trim().parse().ok()?;

        // Convert from 1/10 mm to mm
        let width_mm = width / 10.0;
        let height_mm = height / 10.0;

        // Match to standard sizes
        if (width_mm - 215.9).abs() < 5.0 && (height_mm - 279.4).abs() < 5.0 {
            return Some(PaperSize::Letter);
        }
        if (width_mm - 210.0).abs() < 5.0 && (height_mm - 297.0).abs() < 5.0 {
            return Some(PaperSize::A4);
        }
        if (width_mm - 215.9).abs() < 5.0 && (height_mm - 355.6).abs() < 5.0 {
            return Some(PaperSize::Legal);
        }
    }

    Some(PaperSize::Letter) // Default
}

fn parse_font_directive(content: &str) -> Option<(String, FontSpec)> {
    // Format: Font:CardTable "Arial",11,400,0
    let colon_pos = content.find(':')?;
    let rest = &content[colon_pos + 1..];

    let space_pos = rest.find(' ')?;
    let name = rest[..space_pos].to_string();
    let params = &rest[space_pos + 1..];

    // Parse "FontFamily",size,weight,italic
    let parts: Vec<&str> = params.split(',').collect();
    if parts.len() < 4 {
        return None;
    }

    let family = parts[0].trim().trim_matches('"').to_string();
    let size: f32 = parts[1].trim().parse().ok()?;
    let weight: u16 = parts[2].trim().parse().ok()?;
    let italic = parts[3].trim() != "0";

    Some((
        name,
        FontSpec {
            family,
            size,
            weight,
            italic,
        },
    ))
}

fn parse_pip_colors(value: &str) -> Option<ColorSettings> {
    // Format: "#000000,#ff0000,#ff0000,#000000" (spades, hearts, diamonds, clubs)
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 4 {
        return None;
    }

    Some(ColorSettings {
        spades: parse_color(parts[0])?,
        hearts: parse_color(parts[1])?,
        diamonds: parse_color(parts[2])?,
        clubs: parse_color(parts[3])?,
    })
}

fn parse_color(value: &str) -> Option<(u8, u8, u8)> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&value[0..2], 16).ok()?;
    let g = u8::from_str_radix(&value[2..4], 16).ok()?;
    let b = u8::from_str_radix(&value[4..6], 16).ok()?;

    Some((r, g, b))
}

/// Parse %Translate directive: "Board %" "%)"
/// Returns (from_pattern, to_pattern) if successful
fn parse_translate(value: &str) -> Option<(String, String)> {
    // Format: "from" "to" - two quoted strings
    let value = value.trim();

    // Find first quoted string
    let first_start = value.find('"')?;
    let first_end = value[first_start + 1..].find('"')? + first_start + 1;
    let from = value[first_start + 1..first_end].to_string();

    // Find second quoted string
    let rest = &value[first_end + 1..];
    let second_start = rest.find('"')?;
    let second_end = rest[second_start + 1..].find('"')? + second_start + 1;
    let to = rest[second_start + 1..second_end].to_string();

    Some((from, to))
}

/// Parse BCOptions line: "Float Justify NoHRStats STBorder STShade ShowHCP Center TwoColAuctions"
fn parse_bc_options(value: &str) -> BCOptions {
    let mut options = BCOptions::default();

    // Split by whitespace and check for each option
    for word in value.split_whitespace() {
        match word {
            "Justify" => options.justify = true,
            "ShowHCP" => options.show_hcp = true,
            "Float" => options.float = true,
            "Center" => options.center = true,
            "TwoColAuctions" => options.two_col_auctions = true,
            _ => {} // Ignore unknown options like NoHRStats, STBorder, STShade, GutterH, GutterV, PageHeader
        }
    }

    options
}

/// Parse all header lines and build metadata
pub fn parse_headers(lines: &[&str]) -> PbnMetadata {
    let mut metadata = PbnMetadata::default();

    for line in lines {
        if let Some(directive) = parse_header_line(line) {
            match directive {
                HeaderDirective::Version(v) => metadata.version = Some(v),
                HeaderDirective::Creator(c) => metadata.creator = Some(c),
                HeaderDirective::Created(c) => metadata.created = Some(c),
                HeaderDirective::BoardsPerPage(config) => {
                    metadata.layout.boards_per_page = Some(config.count);
                    if config.count >= 2 && config.fit {
                        metadata.layout.column_count = config.count;
                    }
                }
                HeaderDirective::Margins(m) => metadata.layout.margins = Some(m),
                HeaderDirective::PaperSize(s) => metadata.layout.paper_size = Some(s),
                HeaderDirective::Font(name, spec) => match name.as_str() {
                    "CardTable" => metadata.fonts.card_table = Some(spec),
                    "Commentary" => metadata.fonts.commentary = Some(spec),
                    "Diagram" => metadata.fonts.diagram = Some(spec),
                    "Event" => metadata.fonts.event = Some(spec),
                    "FixedPitch" => metadata.fonts.fixed_pitch = Some(spec),
                    "HandRecord" => metadata.fonts.hand_record = Some(spec),
                    _ => {}
                },
                HeaderDirective::PipColors(c) => metadata.colors = c,
                HeaderDirective::TitleEvent(t) => metadata.title_event = Some(t),
                HeaderDirective::TitleDate(d) => metadata.title_date = Some(d),
                HeaderDirective::ShowHcp(v) => metadata.layout.show_hcp = v,
                HeaderDirective::ShowCardTable(v) => metadata.layout.show_card_table = Some(v),
                HeaderDirective::ShowBoardLabels(v) => metadata.layout.show_board_labels = Some(v),
                HeaderDirective::BCOptions(opts) => {
                    if opts.show_hcp {
                        metadata.layout.show_hcp = true;
                    }
                    if opts.justify {
                        metadata.layout.justify = true;
                    }
                    if opts.center {
                        metadata.layout.center = true;
                    }
                    if opts.two_col_auctions {
                        metadata.layout.two_col_auctions = true;
                    }
                }
                HeaderDirective::BoardLabelFormat(fmt) => {
                    metadata.layout.board_label_format = Some(fmt);
                }
                HeaderDirective::Unknown(_) => {}
            }
        }
    }

    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let directive = parse_header_line("% PBN 2.1").unwrap();
        match directive {
            HeaderDirective::Version(v) => assert_eq!(v, "2.1"),
            _ => panic!("Expected Version directive"),
        }
    }

    #[test]
    fn test_parse_boards_per_page() {
        let directive = parse_header_line("%BoardsPerPage fit,1").unwrap();
        match directive {
            HeaderDirective::BoardsPerPage(config) => {
                assert_eq!(config.count, 1);
                assert!(config.fit);
            }
            _ => panic!("Expected BoardsPerPage directive"),
        }
    }

    #[test]
    fn test_parse_boards_per_page_two_column() {
        let directive = parse_header_line("%BoardsPerPage fit,2").unwrap();
        match directive {
            HeaderDirective::BoardsPerPage(config) => {
                assert_eq!(config.count, 2);
                assert!(config.fit);
            }
            _ => panic!("Expected BoardsPerPage directive"),
        }
    }

    #[test]
    fn test_parse_margins() {
        // Format: top, bottom, right, left (1000 twips = 1 inch = 25.4mm)
        let directive = parse_header_line("%Margins 1000,1000,500,750").unwrap();
        match directive {
            HeaderDirective::Margins(m) => {
                assert!((m.top - 25.4).abs() < 0.1); // 1000 twips = 1 inch
                assert!((m.bottom - 25.4).abs() < 0.1); // 1000 twips = 1 inch
                assert!((m.right - 12.7).abs() < 0.1); // 500 twips = 0.5 inch
                assert!((m.left - 19.05).abs() < 0.1); // 750 twips = 0.75 inch
            }
            _ => panic!("Expected Margins directive"),
        }
    }

    #[test]
    fn test_parse_pip_colors() {
        let directive = parse_header_line("%PipColors #000000,#ff0000,#ff0000,#000000").unwrap();
        match directive {
            HeaderDirective::PipColors(c) => {
                assert_eq!(c.spades, (0, 0, 0));
                assert_eq!(c.hearts, (255, 0, 0));
            }
            _ => panic!("Expected PipColors directive"),
        }
    }

    #[test]
    fn test_parse_bc_options() {
        let directive =
            parse_header_line("%BCOptions Float Justify NoHRStats STBorder STShade ShowHCP")
                .unwrap();
        match directive {
            HeaderDirective::BCOptions(opts) => {
                assert!(opts.justify, "Expected Justify to be true");
                assert!(opts.show_hcp, "Expected ShowHCP to be true");
                assert!(opts.float, "Expected Float to be true");
            }
            _ => panic!("Expected BCOptions directive"),
        }
    }

    #[test]
    fn test_parse_bc_options_partial() {
        let directive = parse_header_line("%BCOptions Justify").unwrap();
        match directive {
            HeaderDirective::BCOptions(opts) => {
                assert!(opts.justify, "Expected Justify to be true");
                assert!(!opts.show_hcp, "Expected ShowHCP to be false");
                assert!(!opts.float, "Expected Float to be false");
            }
            _ => panic!("Expected BCOptions directive"),
        }
    }

    #[test]
    fn test_parse_bc_options_two_col_auctions() {
        // Test from Stayman exercises file
        let directive = parse_header_line(
            "%BCOptions Center GutterH GutterV Justify NoHRStats PageHeader STBorder STShade TwoColAuctions",
        )
        .unwrap();
        match directive {
            HeaderDirective::BCOptions(opts) => {
                assert!(opts.justify, "Expected Justify to be true");
                assert!(opts.center, "Expected Center to be true");
                assert!(opts.two_col_auctions, "Expected TwoColAuctions to be true");
            }
            _ => panic!("Expected BCOptions directive"),
        }
    }

    #[test]
    fn test_parse_headers_two_col_auctions() {
        let lines = vec!["%BCOptions TwoColAuctions"];
        let metadata = parse_headers(&lines);
        assert!(
            metadata.layout.two_col_auctions,
            "Expected two_col_auctions to be true in metadata"
        );
    }

    #[test]
    fn test_parse_translate_board_label() {
        let directive = parse_header_line("%Translate \"Board %\" \"%)\"").unwrap();
        match directive {
            HeaderDirective::BoardLabelFormat(fmt) => {
                assert_eq!(fmt, "%)");
            }
            _ => panic!("Expected BoardLabelFormat directive"),
        }
    }

    #[test]
    fn test_parse_translate_in_headers() {
        let lines = vec!["%Translate \"Board %\" \"%)\""];
        let metadata = parse_headers(&lines);
        assert_eq!(metadata.layout.board_label_format, Some("%)".to_string()));
    }

    #[test]
    fn show_card_table_and_board_labels_stay_unset_unless_the_file_says() {
        let metadata = parse_headers(&["%ShowCardTable 0", "%ShowBoardLabels 2"]);
        assert_eq!(metadata.layout.show_card_table, Some(false));
        assert_eq!(metadata.layout.show_board_labels, Some(true));

        let metadata = parse_headers(&["%BCOptions Justify"]);
        assert_eq!(metadata.layout.show_card_table, None);
        assert_eq!(metadata.layout.show_board_labels, None);
    }
}
