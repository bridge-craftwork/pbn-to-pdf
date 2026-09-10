//! Reading a PBN file into this crate's render model.
//!
//! The parsing itself belongs to `bridge-encodings`, which is the org's PBN
//! reader: it is section-aware, keeps `{...}` commentary, preserves every tag it
//! does not model in `extra_tags`, and handles the producer quirks this crate
//! used to carry its own parser for. What is left here is the adaptation from
//! `bridge_types::Board` to [`crate::model::Board`] — the render model, which
//! holds a few things a general reader has no business knowing about:
//! per-suit holdings in display order, rich commentary spans, Bridge Composer
//! display flags, and a typed contract.

use bridge_encodings::pbn::read_pbn;

use crate::error::PbnError;
use crate::model::{
    AnnotatedCall, Auction, BCFlags, Board, Call, Contract, Deal, Hand, HiddenHands, Holding,
    PbnMetadata, PlaySequence, Trick,
};

use super::commentary::parse_commentary;
use super::header::parse_headers;

/// Result of parsing a PBN file
#[derive(Debug)]
pub struct PbnFile {
    pub metadata: PbnMetadata,
    pub boards: Vec<Board>,
}

/// Parse a complete PBN file
pub fn parse_pbn(content: &str) -> Result<PbnFile, PbnError> {
    // `%` directives are read from the raw text rather than from the boards'
    // `directives`, which anchor each line to the record it sits in. Bridge
    // Composer writes its page setup in a file header, but not exclusively, and
    // the settings are file-wide wherever they appear.
    let header_lines: Vec<&str> = content
        .lines()
        .filter(|line| line.trim().starts_with('%'))
        .collect();
    let metadata = parse_headers(&header_lines);

    let boards = read_pbn(content)
        .map_err(|e| PbnError::ParseError(e.to_string()))?
        .into_iter()
        .map(adapt_board)
        .collect();

    Ok(PbnFile { metadata, boards })
}

/// Build the render model's board from the one the reader produced.
fn adapt_board(src: bridge_types::Board) -> Board {
    let mut board = Board {
        number: src.number,
        board_id: src.board_id,
        event: src.event,
        site: src.site,
        date: src.date,
        dealer: src.dealer,
        vulnerable: src.vulnerable,
        deal: adapt_deal(&src.deal),
        players: src.player_names.unwrap_or_default(),
        auction: src.auction.map(adapt_auction),
        contract: None,
        declarer: src.declarer,
        play: src.play.map(adapt_play),
        result: src.result,
        commentary: src
            .commentary
            .iter()
            .filter_map(|text| parse_commentary(text).ok())
            .collect(),
        bc_flags: None,
        hidden: HiddenHands::default(),
    };

    // The declarer names the contract's declarer whichever tag came first.
    board.contract = src
        .contract
        .as_deref()
        .and_then(Contract::parse)
        .map(|mut c| {
            if let Some(declarer) = src.declarer {
                c.declarer = declarer;
            }
            c
        });

    // Bridge Composer's display tags are not part of any PBN standard, so the
    // reader leaves them where it leaves every unmodeled tag.
    for (name, value) in &src.extra_tags {
        match name.as_str() {
            "BCFlags" => board.bc_flags = BCFlags::from_hex(value),
            "Hidden" => board.hidden = HiddenHands::from_pbn(value),
            _ => {}
        }
    }

    board
}

/// Regroup the four hands into per-suit holdings, which is how every diagram,
/// fan and dummy layout reads a hand.
fn adapt_deal(src: &bridge_types::Deal) -> Deal {
    let mut deal = Deal::new();
    for direction in bridge_types::Direction::ALL {
        deal.set_hand(direction, adapt_hand(src.hand(direction)));
    }
    deal
}

fn adapt_hand(src: &bridge_types::Hand) -> Hand {
    use crate::model::Suit;

    let holding = |suit: Suit| {
        Holding::from_ranks(
            src.cards()
                .iter()
                .filter(|card| card.suit == suit)
                .map(|card| card.rank),
        )
    };
    Hand::from_holdings(
        holding(Suit::Spades),
        holding(Suit::Hearts),
        holding(Suit::Diamonds),
        holding(Suit::Clubs),
    )
}

fn adapt_auction(src: bridge_types::Auction) -> Auction {
    let mut auction = Auction::new(src.dealer);
    auction.notes = src.notes;
    auction.calls = src
        .calls
        .iter()
        .map(|ac| AnnotatedCall {
            call: ac.call.clone(),
            annotation: ac.annotation.as_deref().and_then(display_annotation),
        })
        .collect();

    auction.is_passed_out =
        auction.calls.len() >= 4 && auction.calls[..4].iter().all(|ac| ac.call == Call::Pass);
    auction
}

/// What an annotation should read as on the page.
///
/// The reader keeps annotations exactly as the file wrote them, since it has to
/// write them back. Rendering wants the part a reader sees: the number out of a
/// `=n=` note reference, the marker itself for `!`/`?`, and the conventional
/// text for a numeric annotation glyph (PBN 3.6: `$1` is "!", `$2` is "?").
///
/// A call can collect more than one, either because the file glued a marker to
/// it and then wrote a note reference after it (`P? =7=`) or because it wrote
/// two references for one call (`3NT =4= or =5=`). Only one can be shown, and
/// it is the last legible one — the file's own last word on that call.
fn display_annotation(annotation: &str) -> Option<String> {
    annotation_parts(annotation)
        .into_iter()
        .filter_map(display_one)
        .next_back()
}

/// Split a run of annotations into the individual ones. Anything unrecognized
/// is returned as a part of its own so it can be discarded rather than swallow
/// what follows it.
fn annotation_parts(annotation: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut rest = annotation;
    while !rest.is_empty() {
        let take = match rest.as_bytes()[0] {
            // `=n=`, up to and including its closing `=`.
            b'=' => rest[1..].find('=').map(|at| at + 2).unwrap_or(rest.len()),
            // `$` and the digits after it.
            b'$' => {
                1 + rest[1..]
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(rest.len() - 1)
            }
            _ => 1,
        };
        let (part, tail) = rest.split_at(take.min(rest.len()));
        parts.push(part);
        rest = tail;
    }
    parts
}

fn display_one(part: &str) -> Option<String> {
    if let Some(inner) = part.strip_prefix('=').and_then(|r| r.strip_suffix('=')) {
        return inner.parse::<u8>().ok().map(|n| n.to_string());
    }
    match part {
        "!" | "?" => Some(part.to_string()),
        "$1" => Some("!".to_string()),
        "$2" => Some("?".to_string()),
        "$3" => Some("!!".to_string()),
        "$4" => Some("??".to_string()),
        _ => None,
    }
}

fn adapt_play(src: bridge_types::PlaySequence) -> PlaySequence {
    let mut play = PlaySequence::new(src.opening_leader);
    for src_trick in &src.tricks {
        let mut trick = Trick::new(src_trick.leader);
        for (position, card) in src_trick.cards.iter().enumerate() {
            if let Some(card) = card {
                trick.set_card(position, *card);
            }
        }
        trick.winner = src_trick.winner;
        play.add_trick(trick);
    }
    play
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Direction, Strain, Suit, Vulnerability};

    fn parse(content: &str) -> PbnFile {
        parse_pbn(content).expect("parses")
    }

    fn auction_of(content: &str) -> Auction {
        parse(content).boards.remove(0).auction.expect("an auction")
    }

    #[test]
    fn a_board_carries_its_setup_and_its_hands() {
        let file = parse(
            "% PBN 2.1\n\
             [Event \"Test\"]\n\
             [Board \"1\"]\n\
             [Dealer \"N\"]\n\
             [Vulnerable \"None\"]\n\
             [Deal \"N:AKQ.JT9.876.5432 JT9.AKQ.543.8765 876.543.AKQ.JT98 543.876.JT9.AKQ6\"]\n",
        );
        let board = &file.boards[0];
        assert_eq!(board.number, Some(1));
        assert_eq!(board.dealer, Some(Direction::North));
        assert_eq!(board.vulnerable, Vulnerability::None);
        assert_eq!(board.deal.north.spades.len(), 3);
        // Holdings arrive in display order, whatever order the file used.
        assert_eq!(board.deal.north.hearts.ranks[0], crate::model::Rank::Jack);
        assert_eq!(board.deal.south.holding(Suit::Diamonds).len(), 3);
    }

    #[test]
    fn a_board_id_keeps_the_form_it_was_written_in() {
        // Lesson sets number boards "1-1"; `number` cannot hold that, and the
        // page prints the id.
        let board = parse("[Board \"3-2\"]\n[Dealer \"N\"]\n").boards.remove(0);
        assert_eq!(board.board_id.as_deref(), Some("3-2"));
        assert_eq!(board.number, None);
    }

    #[test]
    fn boards_are_separated_by_their_records() {
        let file = parse(
            "[Event \"One\"]\n[Board \"1\"]\n[Dealer \"N\"]\n\n\
             [Event \"Two\"]\n[Board \"2\"]\n[Dealer \"E\"]\n",
        );
        assert_eq!(file.boards.len(), 2);
        assert_eq!(file.boards[1].number, Some(2));
    }

    #[test]
    fn all_pass_is_the_three_passes_it_stands_for() {
        let auction = auction_of("[Board \"1\"]\n[Auction \"N\"]\n1D 1S X Pass 1NT AP\n");
        assert_eq!(auction.calls.len(), 8);
        assert_eq!(auction.calls[0].call, Call::bid(1, Strain::Diamonds));
        assert!(auction.calls[5..].iter().all(|c| c.call == Call::Pass));
    }

    #[test]
    fn a_passed_out_auction_says_so() {
        let auction = auction_of("[Board \"1\"]\n[Auction \"S\"]\nPass Pass Pass Pass\n");
        assert!(auction.is_passed_out);
        assert_eq!(auction.calls.len(), 4);
    }

    #[test]
    fn an_alert_marker_annotates_its_call_without_displacing_it() {
        // The regression this guards: a call that fails to parse is dropped,
        // and every call after it moves one seat.
        let auction = auction_of("[Board \"1\"]\n[Auction \"W\"]\n1C! 1H 2C Pass\n");
        assert_eq!(auction.calls.len(), 4);
        assert_eq!(auction.calls[0].call, Call::bid(1, Strain::Clubs));
        assert_eq!(auction.calls[0].annotation.as_deref(), Some("!"));
        assert_eq!(auction.calls[1].call, Call::bid(1, Strain::Hearts));
    }

    #[test]
    fn a_note_reference_shows_as_its_number_and_resolves_to_its_text() {
        let auction = auction_of(
            "[Board \"1\"]\n[Auction \"E\"]\nPass 2NT =1= Pass\n[Note \"1:20-21 balanced\"]\n",
        );
        assert_eq!(auction.calls.len(), 3, "a reference is not a call");
        assert_eq!(auction.calls[1].annotation.as_deref(), Some("1"));
        assert_eq!(
            auction.notes.get(&1).map(String::as_str),
            Some("20-21 balanced")
        );
    }

    #[test]
    fn the_last_legible_annotation_on_a_call_is_the_one_shown() {
        // Files write both `3NT =4= or =5=` and `P? =7=`. Only one annotation
        // fits above a call, and the file's last word on it wins.
        let auction = auction_of("[Board \"1\"]\n[Auction \"N\"]\n3NT =4= or =5= Pass\n");
        assert_eq!(auction.calls[0].annotation.as_deref(), Some("5"));

        let auction = auction_of("[Board \"1\"]\n[Auction \"N\"]\n1NT Pass P? =7= Pass\n");
        assert_eq!(auction.calls[2].call, Call::Pass);
        assert_eq!(auction.calls[2].annotation.as_deref(), Some("7"));
    }

    #[test]
    fn a_numeric_annotation_glyph_reads_as_its_marker() {
        // `$2` is "?" (PBN 3.6). On a worksheet's blank it is the prompt.
        let auction = auction_of("[Board \"1\"]\n[Auction \"N\"]\n1NT Pass ___ $2\n");
        assert_eq!(auction.calls[2].call, Call::Blank);
        assert_eq!(auction.calls[2].annotation.as_deref(), Some("?"));
    }

    #[test]
    fn a_continued_auction_ends_in_the_students_turn() {
        // `+` closes the section for a reader; the worksheet draws it as "?".
        let auction = auction_of("[Board \"1\"]\n[Auction \"W\"]\n1D X Pass +\n");
        assert_eq!(auction.calls.len(), 4);
        assert_eq!(auction.calls[3].call, Call::Continue);
    }

    #[test]
    fn an_unknown_hand_does_not_cost_the_rest_of_the_deal() {
        // Two-hand bidding records are written this way.
        let board = parse("[Board \"1\"]\n[Deal \"W:- K43.AQJ54.63.T95 - A5.KT83.K752.843\"]\n")
            .boards
            .remove(0);
        assert_eq!(board.deal.west.card_count(), 0);
        assert_eq!(board.deal.north.card_count(), 13);
        assert_eq!(board.deal.south.card_count(), 13);
    }

    #[test]
    fn a_play_section_written_on_the_tag_line_still_gives_the_lead() {
        // `[Play "W"]SJ` — the opening lead jammed onto the tag line. Dropping
        // it left a declarer's plan with no lead box and no complaint.
        let board = parse("[Board \"1\"]\n[Play \"W\"]SJ\nH2 D3 C4\n")
            .boards
            .remove(0);
        let play = board.play.expect("a play sequence");
        assert_eq!(play.opening_leader, Direction::West);
        assert_eq!(
            play.tricks[0].cards[0],
            Some(crate::model::Card::new(
                Suit::Spades,
                crate::model::Rank::Jack
            ))
        );
    }

    #[test]
    fn the_contract_takes_its_declarer_from_the_declarer_tag() {
        for order in [
            "[Contract \"4S\"]\n[Declarer \"E\"]\n",
            "[Declarer \"E\"]\n[Contract \"4S\"]\n",
        ] {
            let board = parse(&format!("[Board \"1\"]\n{order}")).boards.remove(0);
            let contract = board.contract.expect("a contract");
            assert_eq!(contract.level, 4);
            assert_eq!(contract.suit, Strain::Spades);
            assert_eq!(contract.declarer, Direction::East);
        }
    }

    #[test]
    fn bridge_composer_display_tags_are_read_off_the_board() {
        let board = parse("[Board \"1\"]\n[BCFlags \"1f\"]\n[Hidden \"NS\"]\n")
            .boards
            .remove(0);
        assert!(board.bc_flags.is_some());
        assert!(board.hidden.north && board.hidden.south);
        assert!(!board.hidden.east);
    }

    #[test]
    fn commentary_keeps_the_shape_its_author_gave_it() {
        // Indentation inside a block is the author's, and the renderer lays out
        // what it is given: trimming each line silently reflows the page.
        let board = parse("[Board \"1\"]\n{First line\n  second line}\n")
            .boards
            .remove(0);
        assert_eq!(board.commentary.len(), 1);
        let text = board.commentary[0].content.to_plain_text();
        // The newline reads as a space; the author's indent survives it.
        assert!(text.contains("line   second"), "got {text:?}");
    }

    #[test]
    fn header_directives_are_read_wherever_they_sit() {
        let file = parse("%BCOptions Justify ShowHCP\n[Board \"1\"]\n[Dealer \"N\"]\n");
        assert!(file.metadata.layout.justify);
        assert!(file.metadata.layout.show_hcp);
    }
}
