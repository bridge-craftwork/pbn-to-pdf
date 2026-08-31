use crate::error::PbnError;
use crate::model::{BCFlags, Board, Contract, Direction, HiddenHands, PbnMetadata, Vulnerability};

use super::auction::parse_auction;
use super::commentary::{extract_commentary, parse_commentary};
use super::deal::parse_deal;
use super::header::parse_headers;
use super::play::parse_play;
use super::tags::{parse_tag_pair, TagPair};

/// Parse a note value in format "N:text" where N is the note number
/// Returns (note_number, note_text) if successful
fn parse_note_value(value: &str) -> Option<(u8, String)> {
    let colon_pos = value.find(':')?;
    let num_str = &value[..colon_pos];
    let text = &value[colon_pos + 1..];
    let num = num_str.parse::<u8>().ok()?;
    Some((num, text.to_string()))
}

/// Result of parsing a PBN file
#[derive(Debug)]
pub struct PbnFile {
    pub metadata: PbnMetadata,
    pub boards: Vec<Board>,
}

/// Parse a complete PBN file
pub fn parse_pbn(content: &str) -> Result<PbnFile, PbnError> {
    let lines: Vec<&str> = content.lines().collect();

    // Extract header lines (starting with %)
    let header_lines: Vec<&str> = lines
        .iter()
        .filter(|line| line.trim().starts_with('%'))
        .copied()
        .collect();

    let metadata = parse_headers(&header_lines);

    // Parse boards
    let boards = parse_boards(&lines)?;

    Ok(PbnFile { metadata, boards })
}

/// Parse all board records from the file
fn parse_boards(lines: &[&str]) -> Result<Vec<Board>, PbnError> {
    let mut boards = Vec::new();
    let mut current_board: Option<Board> = None;
    let mut in_auction = false;
    let mut auction_dealer: Option<Direction> = None;
    let mut auction_lines = Vec::new();
    let mut in_play = false;
    let mut play_leader: Option<Direction> = None;
    let mut play_lines = Vec::new();
    let mut in_commentary = false;
    let mut commentary_lines: Vec<&str> = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        // Skip empty lines and comments (but not if we're in commentary)
        if !in_commentary
            && (trimmed.is_empty() || trimmed.starts_with('%') || trimmed.starts_with(';'))
        {
            continue;
        }

        // Handle multi-line commentary
        if in_commentary {
            commentary_lines.push(*line);
            if line.contains('}') {
                // End of commentary block
                in_commentary = false;
                if let Some(ref mut board) = current_board {
                    let full_text = commentary_lines.join("\n");
                    if let Some((commentary_text, _)) = extract_commentary(&full_text) {
                        if let Ok(block) = parse_commentary(commentary_text) {
                            board.commentary.push(block);
                        }
                    }
                }
                commentary_lines.clear();
            }
            continue;
        }

        // Check for tag pairs
        if trimmed.starts_with('[') {
            // Finish any ongoing auction section
            if in_auction && !auction_lines.is_empty() {
                if let (Some(ref mut board), Some(dealer)) = (&mut current_board, auction_dealer) {
                    let auction_text = auction_lines.join(" ");
                    if let Ok(auction) = parse_auction(dealer, &auction_text) {
                        board.auction = Some(auction);
                    }
                }
                auction_lines.clear();
                in_auction = false;
            }

            // Finish any ongoing play section
            if in_play && !play_lines.is_empty() {
                if let (Some(ref mut board), Some(leader)) = (&mut current_board, play_leader) {
                    let play_text = play_lines.join(" ");
                    if let Ok(play) = parse_play(leader, &play_text) {
                        board.play = Some(play);
                    }
                }
                play_lines.clear();
                in_play = false;
            }

            // Parse the tag pair
            if let Ok((rest, tag)) = parse_tag_pair(trimmed) {
                let name = tag.name.clone();
                process_tag(
                    &mut current_board,
                    &mut boards,
                    tag,
                    &mut in_auction,
                    &mut auction_dealer,
                    &mut in_play,
                    &mut play_leader,
                )?;

                // A section's data belongs on the lines after its tag pair, but
                // some producers write the first of it on the tag's own line --
                // `[Play "W"]SJ`. Discarding the remainder loses exactly one
                // datum, and for a Play section that datum is the opening lead,
                // so the loss is silent and total. Take it, and say so.
                let rest = rest.trim();
                if !rest.is_empty() {
                    if in_auction || in_play {
                        log::warn!(
                            "[{name}] section data on the tag line ({rest:?}); \
                             the PBN standard puts it on the following line"
                        );
                        if in_auction {
                            auction_lines.push(rest);
                        } else {
                            play_lines.push(rest);
                        }
                    } else {
                        log::debug!("ignoring trailing text after [{name}]: {rest:?}");
                    }
                }
            }
        } else if trimmed.starts_with('{') {
            // Start of commentary block
            commentary_lines.push(*line);
            if line.contains('}') {
                // Single-line commentary
                if let Some(ref mut board) = current_board {
                    if let Some((commentary_text, _)) = extract_commentary(line) {
                        if let Ok(block) = parse_commentary(commentary_text) {
                            board.commentary.push(block);
                        }
                    }
                }
                commentary_lines.clear();
            } else {
                // Multi-line commentary
                in_commentary = true;
            }
        } else if in_auction {
            // Continuation of auction section
            auction_lines.push(trimmed);
        } else if in_play {
            // Continuation of play section
            play_lines.push(trimmed);
        }
    }

    // Finish any final auction section
    if in_auction && !auction_lines.is_empty() {
        if let (Some(ref mut board), Some(dealer)) = (&mut current_board, auction_dealer) {
            let auction_text = auction_lines.join(" ");
            if let Ok(auction) = parse_auction(dealer, &auction_text) {
                board.auction = Some(auction);
            }
        }
    }

    // Finish any final play section
    if in_play && !play_lines.is_empty() {
        if let (Some(ref mut board), Some(leader)) = (&mut current_board, play_leader) {
            let play_text = play_lines.join(" ");
            if let Ok(play) = parse_play(leader, &play_text) {
                board.play = Some(play);
            }
        }
    }

    // Save the last board
    if let Some(board) = current_board {
        boards.push(board);
    }

    Ok(boards)
}

/// Process a single tag pair
fn process_tag(
    current_board: &mut Option<Board>,
    boards: &mut Vec<Board>,
    tag: TagPair,
    in_auction: &mut bool,
    auction_dealer: &mut Option<Direction>,
    in_play: &mut bool,
    play_leader: &mut Option<Direction>,
) -> Result<(), PbnError> {
    match tag.name.as_str() {
        "Event" => {
            // Start of a new board
            if let Some(board) = current_board.take() {
                boards.push(board);
            }
            let mut board = Board::new();
            if !tag.value.is_empty() {
                board.event = Some(tag.value);
            }
            *current_board = Some(board);
        }
        "Site" => {
            if let Some(ref mut board) = current_board {
                if !tag.value.is_empty() {
                    board.site = Some(tag.value);
                }
            }
        }
        "Date" => {
            if let Some(ref mut board) = current_board {
                if !tag.value.is_empty() {
                    board.date = Some(tag.value);
                }
            }
        }
        "Board" => {
            // BridgeComposer sometimes omits [Event] before the first board.
            // If no current board exists, start a new one so the board's data
            // is captured (Deal, Auction, etc.).
            if current_board.is_none() {
                *current_board = Some(Board::new());
            }
            if let Some(ref mut board) = current_board {
                if !tag.value.is_empty() {
                    board.board_id = Some(tag.value.clone());
                }
                if let Ok(num) = tag.value.parse::<u32>() {
                    board.number = Some(num);
                }
            }
        }
        "Dealer" => {
            if let Some(ref mut board) = current_board {
                if let Some(dir) = tag.value.chars().next().and_then(Direction::from_char) {
                    board.dealer = Some(dir);
                }
            }
        }
        "Vulnerable" => {
            if let Some(ref mut board) = current_board {
                if let Some(vuln) = Vulnerability::from_pbn(&tag.value) {
                    board.vulnerable = vuln;
                }
            }
        }
        "Deal" => {
            if let Some(ref mut board) = current_board {
                match parse_deal(&tag.value) {
                    Ok(deal) => board.deal = deal,
                    Err(e) => {
                        log::warn!("Failed to parse deal: {}", e);
                    }
                }
            }
        }
        "Declarer" => {
            if let Some(ref mut board) = current_board {
                if let Some(dir) = tag.value.chars().next().and_then(Direction::from_char) {
                    board.declarer = Some(dir);
                    // Update contract declarer if contract already exists
                    if let Some(ref mut contract) = board.contract {
                        contract.declarer = dir;
                    }
                }
            }
        }
        "Contract" => {
            if let Some(ref mut board) = current_board {
                if let Some(mut contract) = Contract::parse(&tag.value) {
                    // If declarer was already set, use it
                    if let Some(declarer) = board.declarer {
                        contract.declarer = declarer;
                    }
                    board.contract = Some(contract);
                }
            }
        }
        "Result" => {
            if let Some(ref mut board) = current_board {
                if let Ok(result) = tag.value.parse::<i8>() {
                    board.result = Some(result);
                }
            }
        }
        "Auction" => {
            *in_auction = true;
            *in_play = false;
            if let Some(dir) = tag.value.chars().next().and_then(Direction::from_char) {
                *auction_dealer = Some(dir);
            }
        }
        "Play" => {
            *in_play = true;
            *in_auction = false;
            if let Some(dir) = tag.value.chars().next().and_then(Direction::from_char) {
                *play_leader = Some(dir);
            }
        }
        "Note" => {
            // Parse note in format "N:text" where N is the note number
            if let Some(ref mut board) = current_board {
                if let Some((num, text)) = parse_note_value(&tag.value) {
                    if let Some(ref mut auction) = board.auction {
                        auction.add_note(num, text);
                    }
                }
            }
        }
        "BCFlags" => {
            // Bridge Composer display flags (hexadecimal bitmask)
            if let Some(ref mut board) = current_board {
                if let Some(flags) = BCFlags::from_hex(&tag.value) {
                    board.bc_flags = Some(flags);
                }
            }
        }
        "Hidden" => {
            // Hidden hands (e.g., "NS", "ESW", "NESW")
            if let Some(ref mut board) = current_board {
                board.hidden = HiddenHands::from_pbn(&tag.value);
            }
        }
        "North" => {
            if let Some(ref mut board) = current_board {
                if !tag.value.is_empty() {
                    board.players.north = Some(tag.value);
                }
            }
        }
        "East" => {
            if let Some(ref mut board) = current_board {
                if !tag.value.is_empty() {
                    board.players.east = Some(tag.value);
                }
            }
        }
        "South" => {
            if let Some(ref mut board) = current_board {
                if !tag.value.is_empty() {
                    board.players.south = Some(tag.value);
                }
            }
        }
        "West" => {
            if let Some(ref mut board) = current_board {
                if !tag.value.is_empty() {
                    board.players.west = Some(tag.value);
                }
            }
        }
        _ => {
            // Unknown tag, skip
            log::debug!("Skipping unknown tag: {}", tag.name);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pbn() {
        let content = r#"% PBN 2.1
[Event "Test"]
[Site ""]
[Date "2024.01.01"]
[Board "1"]
[Dealer "N"]
[Vulnerable "None"]
[Deal "N:AKQ.JT9.876.5432 JT9.AKQ.543.8765 876.543.AKQ.JT98 543.876.JT9.AKQ6"]
[Auction "N"]
1NT Pass 3NT AP
"#;

        let result = parse_pbn(content).unwrap();
        assert_eq!(result.boards.len(), 1);

        let board = &result.boards[0];
        assert_eq!(board.number, Some(1));
        assert_eq!(board.dealer, Some(Direction::North));
        assert_eq!(board.vulnerable, Vulnerability::None);
        assert_eq!(board.deal.north.spades.len(), 3); // AKQ
    }

    #[test]
    fn test_parse_multiple_boards() {
        let content = r#"[Event "Test1"]
[Board "1"]
[Dealer "N"]
[Vulnerable "None"]
[Deal "N:AKQ.JT9.876.5432 JT9.AKQ.543.8765 876.543.AKQ.JT98 543.876.JT9.AKQ6"]

[Event "Test2"]
[Board "2"]
[Dealer "E"]
[Vulnerable "NS"]
[Deal "N:AKQ.JT9.876.5432 JT9.AKQ.543.8765 876.543.AKQ.JT98 543.876.JT9.AKQ6"]
"#;

        let result = parse_pbn(content).unwrap();
        assert_eq!(result.boards.len(), 2);
        assert_eq!(result.boards[0].number, Some(1));
        assert_eq!(result.boards[1].number, Some(2));
        assert_eq!(result.boards[1].vulnerable, Vulnerability::NorthSouth);
    }

    #[test]
    fn test_parse_bcflags() {
        let content = r#"[Event "Test"]
[Board "1"]
[Dealer "N"]
[Vulnerable "None"]
[Deal "N:AKQ.JT9.876.5432 JT9.AKQ.543.8765 876.543.AKQ.JT98 543.876.JT9.AKQ6"]
[BCFlags "60001b"]
"#;

        let result = parse_pbn(content).unwrap();
        assert_eq!(result.boards.len(), 1);

        let board = &result.boards[0];
        assert!(board.bc_flags.is_some());

        let flags = board.bc_flags.unwrap();
        assert!(flags.show_play());
        assert!(flags.show_results());
        assert!(flags.show_diagram());
        assert!(flags.show_auction());
        assert!(!flags.hide_board());
        assert!(flags.hide_dealer());
        assert!(flags.hide_vulnerable());
    }

    #[test]
    fn test_parse_hidden() {
        let content = r#"[Event "Test"]
[Board "1"]
[Dealer "N"]
[Vulnerable "None"]
[Deal "N:AKQ.JT9.876.5432 JT9.AKQ.543.8765 876.543.AKQ.JT98 543.876.JT9.AKQ6"]
[Hidden "NS"]
"#;

        let result = parse_pbn(content).unwrap();
        assert_eq!(result.boards.len(), 1);

        let board = &result.boards[0];
        assert!(board.hidden.north);
        assert!(!board.hidden.east);
        assert!(board.hidden.south);
        assert!(!board.hidden.west);
    }

    /// Some producers write a section's first datum on the tag pair's own line
    /// instead of the line below it. The PBN standard puts it below; accepting
    /// it anyway costs nothing and the intent is unambiguous.
    ///
    /// Baker Bridge does this for every one of its Play sections, which made
    /// every opening lead in that collection invisible — the tag parsed, the
    /// card was discarded, and a declarer's plan rendered with no lead and no
    /// complaint.
    mod section_data_on_the_tag_line {
        use super::*;

        const DEAL: &str = concat!(
            "[Board \"1\"]\n",
            "[Dealer \"N\"]\n",
            "[Declarer \"S\"]\n",
            "[Contract \"3NT\"]\n",
            "[Deal \"N:AKQ.JT9.876.5432 T98.876.5432.AKQ 7654.5432.AKQ.JT9 J32.AKQ.JT9.876\"]\n",
        );

        fn first_lead(pbn: &str) -> Option<String> {
            let file = parse_pbn(pbn).unwrap();
            let board = file.boards.first()?;
            let play = board.play.as_ref()?;
            let card = play.tricks.first()?.cards[0]?;
            Some(format!("{:?}{:?}", card.suit, card.rank))
        }

        #[test]
        fn a_play_card_on_the_tag_line_is_still_the_opening_lead() {
            let inline = format!("{DEAL}[Play \"W\"]SJ\n");
            let standard = format!("{DEAL}[Play \"W\"]\nSJ\n");
            assert!(
                first_lead(&inline).is_some(),
                "inline play data was dropped"
            );
            assert_eq!(first_lead(&inline), first_lead(&standard));
        }

        #[test]
        fn an_auction_call_on_the_tag_line_is_still_the_first_call() {
            let inline = format!("{DEAL}[Auction \"N\"]1NT Pass 3NT Pass\n");
            let standard = format!("{DEAL}[Auction \"N\"]\n1NT Pass 3NT Pass\n");
            let calls = |p: &str| {
                parse_pbn(p).unwrap().boards[0]
                    .auction
                    .as_ref()
                    .map(|a| a.calls.len())
            };
            assert_eq!(calls(&inline), calls(&standard));
            assert!(
                calls(&inline).unwrap_or(0) >= 4,
                "inline auction data was dropped"
            );
        }

        /// Only a section tag opens a section, so trailing text elsewhere is not
        /// data and must not be swept into one.
        #[test]
        fn trailing_text_after_an_ordinary_tag_is_ignored() {
            // Not [Event] or [Board]: those begin a new record, which would move
            // the play onto a second board and prove nothing.
            let pbn = concat!(
                "[Board \"1\"]\n",
                "[Dealer \"N\"]\n",
                "[Declarer \"S\"] stray words\n",
                "[Deal \"N:AKQ.JT9.876.5432 T98.876.5432.AKQ 7654.5432.AKQ.JT9 J32.AKQ.JT9.876\"]\n",
                "[Play \"W\"]SJ\n",
            );
            let file = parse_pbn(pbn).unwrap();
            assert_eq!(file.boards.len(), 1, "stray text should not start a record");
            assert!(first_lead(pbn).is_some());
        }

        /// The standard form must keep working exactly as before.
        #[test]
        fn the_standard_form_is_unaffected() {
            let pbn = format!("{DEAL}[Play \"W\"]\nSJ H2\n");
            assert!(first_lead(&pbn).is_some());
        }
    }
}
