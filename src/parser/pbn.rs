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
    AnnotatedCall, Auction, BCFlags, Board, CommentarySlot, Deal, FinalContract, Hand, HiddenHands,
    Holding, PbnMetadata,
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

/// Which part of its board a commentary block belongs to, from how many of the
/// record's tags came before it (see [`CommentarySlot`]).
fn commentary_slot(anchor: Option<usize>, tag_order: &[String]) -> CommentarySlot {
    let Some(before) = anchor else {
        return CommentarySlot::default();
    };
    let at = |name: &str| tag_order.iter().position(|t| t == name);
    let deal = at("Deal");
    // A block that `before` tags preceded stands ahead of the tag at index
    // `before`, so it precedes the tag at `i` exactly when `before <= i`.
    if at("Board").or(deal).is_some_and(|i| before <= i) {
        CommentarySlot::Event
    } else if deal.is_some_and(|i| before <= i) {
        CommentarySlot::BeforeDeal
    } else if deal.is_some_and(|i| before == i + 1) {
        CommentarySlot::Diagram
    } else {
        CommentarySlot::Final
    }
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
        play: src.play,
        result: src.result,
        commentary: src
            .commentary
            .iter()
            .enumerate()
            .filter_map(|(i, text)| {
                let mut block = parse_commentary(text).ok()?;
                block.slot =
                    commentary_slot(src.commentary_anchors.get(i).copied(), &src.tag_order);
                Some(block)
            })
            .collect(),
        bc_flags: None,
        hidden: HiddenHands::default(),
    };

    // The `[Declarer]` tag names the declarer whichever tag came first; South
    // stands in when the board gives a contract without one.
    board.contract = src.contract.as_deref().and_then(|c| {
        FinalContract::from_pbn(c, src.declarer.unwrap_or(crate::model::Direction::South))
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

    // A deal written with `x` spot cards did not parse; read it for display
    if board.deal.is_empty() {
        if let Some((deal, left_out)) = src.unparsed_deal.as_deref().and_then(display_deal) {
            board.deal = deal;
            board.hidden.north |= left_out.north;
            board.hidden.east |= left_out.east;
            board.hidden.south |= left_out.south;
            board.hidden.west |= left_out.west;
        }
    }

    board
}

/// A `[Deal]` the reader could not parse, read for display only: `x` is a
/// spot card whose rank the file does not give, and a hand written `...` or
/// `-` is one the diagram leaves out, which comes back as hidden. `None` when
/// the text is not a deal at all.
fn display_deal(text: &str) -> Option<(Deal, HiddenHands)> {
    use crate::model::{Direction, Rank, Suit};

    let (first, hands) = text.trim().split_once(':')?;
    let mut seat = Direction::from_char(first.trim().chars().next()?)?;
    let hands: Vec<&str> = hands.split_whitespace().collect();
    if hands.len() != 4 {
        return None;
    }

    let mut deal = Deal::new();
    let mut left_out = HiddenHands::default();
    for text in hands {
        if matches!(text, "..." | "-") {
            match seat {
                Direction::North => left_out.north = true,
                Direction::East => left_out.east = true,
                Direction::South => left_out.south = true,
                Direction::West => left_out.west = true,
            }
        } else {
            let suits: Vec<&str> = text.split('.').collect();
            if suits.len() != 4 {
                return None;
            }
            let mut hand = Hand::new();
            let order = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];
            for (suit, cards) in order.into_iter().zip(suits) {
                let holding = hand.holding_mut(suit);
                for c in cards.chars() {
                    match c {
                        'x' | 'X' => holding.unknown += 1,
                        _ => holding.add(Rank::from_char(c)?),
                    }
                }
            }
            deal.set_hand(seat, hand);
        }
        seat = seat.next();
    }
    Some((deal, left_out))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_belongs_to_the_slot_it_stood_in() {
        // Kantar's record shape, with a block in each place one can stand
        let tags: Vec<String> = ["Event", "Board", "SkillPath", "Deal", "Result"]
            .map(String::from)
            .to_vec();
        let slot = |before| commentary_slot(Some(before), &tags);
        assert_eq!(slot(0), CommentarySlot::Event, "before every tag");
        assert_eq!(slot(1), CommentarySlot::Event, "after [Event]");
        assert_eq!(slot(3), CommentarySlot::BeforeDeal, "after [SkillPath]");
        assert_eq!(slot(4), CommentarySlot::Diagram, "straight after [Deal]");
        assert_eq!(slot(5), CommentarySlot::Final, "after [Result]");
        assert_eq!(
            commentary_slot(None, &tags),
            CommentarySlot::Final,
            "position unknown"
        );
    }
}
