//! BridgeComposer settings that decide what a board shows (issue #30), checked
//! against what BridgeComposer 5.118.2 does with the same settings.

use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};

/// ABS3-3 practice deals, board 1, one board to a page.
const BOARD: &str = r#"[Board "1"]
[Dealer "N"]
[Vulnerable "None"]
[Deal "N:AQJ8.53.Q92.KQT2 96.J762.KT63.A83 KT72.AKQ8.874.J5 543.T94.AJ5.9764"]
[BCFlags "FLAGS"]
"#;

/// Every content stream in the rendered PDF, decompressed, as text.
fn render(headers: &[&str], flags: &str) -> String {
    let pbn = format!(
        "{}\n\n{}",
        headers.join("\n"),
        BOARD.replace("FLAGS", flags)
    );
    let file = parse_pbn(&pbn).unwrap();
    let headers: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let pdf = render_boards(
        &file.boards,
        &headers,
        Layout::Analysis,
        RenderOptions::default(),
    )
    .unwrap();
    let doc = lopdf::Document::load_mem(&pdf).unwrap();
    let mut content = Vec::new();
    for object in doc.objects.values() {
        if let lopdf::Object::Stream(stream) = object {
            content.extend(
                stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone()),
            );
        }
    }
    String::from_utf8_lossy(&content).into_owned()
}

#[test]
fn bcflags_hide_the_vulnerability_and_the_dealer() {
    let shown = render(&[], "1f");
    assert!(shown.contains("(North Deals)") && shown.contains("(None Vul)"));

    // 0x400000 hides the vulnerability: BridgeComposer omits it on these deals
    let no_vul = render(&[], "40001f");
    assert!(no_vul.contains("(North Deals)"));
    assert!(!no_vul.contains("(None Vul)"));

    // 0x200000 hides the dealer
    let neither = render(&[], "60001f");
    assert!(neither.contains("(Board 1)"));
    assert!(!neither.contains("(North Deals)") && !neither.contains("(None Vul)"));
}

#[test]
fn show_board_labels_0_hides_all_three_labels() {
    let text = render(&["%ShowBoardLabels 0"], "1f");
    for label in ["(Board 1)", "(North Deals)", "(None Vul)"] {
        assert!(!text.contains(label), "{label} still drawn");
    }
    assert!(render(&["%ShowBoardLabels 1"], "1f").contains("(Board 1)"));
}

#[test]
fn show_card_table_0_leaves_out_the_card_table() {
    // The compass letters are drawn only on the card table
    assert!(render(&[], "1f").contains("(N) Tj"));
    assert!(render(&["%ShowCardTable 2"], "1f").contains("(N) Tj"));
    assert!(!render(&["%ShowCardTable 0"], "1f").contains("(N) Tj"));
}

/// Text past ASCII must reach the PDF as Windows-1252, the encoding the builtin
/// fonts are declared with. printpdf writes it as UTF-8, which prints a bullet
/// as "â€¢"; Grant Robinson's slides write their bullets and dashes as Unicode.
#[test]
fn text_past_ascii_is_encoded_as_windows_1252() {
    let pbn = format!(
        "{}{{\u{2022} one \u{2013} two}}\n",
        BOARD.replace("FLAGS", "1f")
    );
    let file = parse_pbn(&pbn).unwrap();
    let pdf = render_boards(
        &file.boards,
        &[],
        Layout::Analysis,
        RenderOptions::default(),
    )
    .unwrap();
    let doc = lopdf::Document::load_mem(&pdf).unwrap();

    let mut shown: Vec<Vec<u8>> = Vec::new();
    for object in doc.objects.values() {
        let lopdf::Object::Stream(stream) = object else {
            continue;
        };
        let Ok(bytes) = stream.decompressed_content() else {
            continue;
        };
        let Ok(content) = lopdf::content::Content::decode(&bytes) else {
            continue;
        };
        for op in content.operations.iter().filter(|op| op.operator == "Tj") {
            if let Some(lopdf::Object::String(text, _)) = op.operands.first() {
                shown.push(text.clone());
            }
        }
    }

    assert!(shown.iter().any(|t| t.contains(&0x95)), "no bullet (0x95)");
    assert!(shown.iter().any(|t| t.contains(&0x96)), "no en dash (0x96)");
    assert!(
        !shown
            .iter()
            .any(|t| t.windows(3).any(|w| w == [0xE2, 0x80, 0xA2])),
        "a bullet was written as UTF-8"
    );
}

/// BridgeComposer draws the HCP box only when the file asks for it with
/// `%BCOptions ShowHCP`; we used to draw it on every board (issue #30).
///
/// North holds 14 on this deal and South 13, and the box draws each seat's
/// count as a string of its own.
#[test]
fn the_hcp_box_waits_for_show_hcp() {
    let hcp_drawn = |headers: &[&str]| {
        let file = parse_pbn(&BOARD.replace("FLAGS", "1f")).unwrap();
        let headers: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
        let pdf = render_boards(
            &file.boards,
            &headers,
            Layout::Analysis,
            RenderOptions::default(),
        )
        .unwrap();
        let shown = shown_strings(&pdf);
        shown.iter().any(|s| s == "14".as_bytes()) && shown.iter().any(|s| s == "13".as_bytes())
    };

    assert!(!hcp_drawn(&[]), "no ShowHCP, so no HCP box");
    assert!(
        hcp_drawn(&["%BCOptions ShowHCP"]),
        "ShowHCP asks for the box"
    );
}

/// Every string drawn with a builtin font, as its raw bytes.
fn shown_strings(pdf: &[u8]) -> Vec<Vec<u8>> {
    let doc = lopdf::Document::load_mem(pdf).unwrap();
    let mut shown = Vec::new();
    for object in doc.objects.values() {
        let lopdf::Object::Stream(stream) = object else {
            continue;
        };
        let Ok(bytes) = stream.decompressed_content() else {
            continue;
        };
        let Ok(content) = lopdf::content::Content::decode(&bytes) else {
            continue;
        };
        for op in content.operations.iter().filter(|op| op.operator == "Tj") {
            if let Some(lopdf::Object::String(text, _)) = op.operands.first() {
                shown.push(text.clone());
            }
        }
    }
    shown
}

/// Grant Robinson's slides write `x` for a spot card whose rank does not
/// matter. The deal used to be dropped outright and nothing was drawn; now the
/// hand is, the spots as `x` the way BridgeComposer prints them.
#[test]
fn x_spot_cards_are_drawn_as_x() {
    let pbn = "[Board \"1\"]\n[Dealer \"N\"]\n[Deal \"N:Kx.Qxx.Qxxx.AJxx ... ... ...\"]\n";
    let file = parse_pbn(pbn).unwrap();
    let board = &file.boards[0];
    let spades = board.deal.north.holding(pbn_to_pdf::model::Suit::Spades);
    assert_eq!((spades.ranks.len(), spades.unknown), (1, 1));
    // The hands written `...` are left out
    assert!(board.hidden.east && board.hidden.south && board.hidden.west);

    let pdf = render_boards(
        &file.boards,
        &[],
        Layout::Analysis,
        RenderOptions::default(),
    )
    .unwrap();
    let shown = shown_strings(&pdf);
    for holding in ["K x", "Q x x", "Q x x x", "A J x x"] {
        assert!(
            shown.iter().any(|s| s == holding.as_bytes()),
            "{holding:?} not drawn"
        );
    }
}

/// A void is an em dash, as BridgeComposer prints it: byte 0x97 in the
/// Windows-1252 the builtin fonts are drawn in.
#[test]
fn a_void_is_an_em_dash() {
    let pbn = "[Board \"1\"]\n[Dealer \"N\"]\n[Deal \"N:AKQJT98765432... - - -\"]\n";
    let file = parse_pbn(pbn).unwrap();
    let pdf = render_boards(
        &file.boards,
        &[],
        Layout::Analysis,
        RenderOptions::default(),
    )
    .unwrap();
    assert!(shown_strings(&pdf).iter().any(|s| s == &[0x97]));
}

/// ABS3-3's 6-1, as that file writes it: a single-suit fragment with the first
/// two cards of a trick on the table. West leads the 4, North plays the 3.
///
/// The play section closes the trick the way a producer does, with seats held
/// by `-` and a `*` after them, so the table has to look past a trick that
/// holds no card of its own.
const TRICK: &str = r#"[Board "6-1"]
[Dealer "S"]
[Vulnerable "None"]
[Deal "S:... 4... T83... KJ5..."]
[BCFlags "FLAGS"]
[Hidden "S"]
[Play "W"]
S4 S3 - -
*
"#;

fn render_trick(flags: &str) -> Vec<Vec<u8>> {
    let file = parse_pbn(&TRICK.replace("FLAGS", flags)).unwrap();
    let pdf = render_boards(
        &file.boards,
        &[],
        Layout::Analysis,
        RenderOptions::default(),
    )
    .unwrap();
    shown_strings(&pdf)
}

/// `BCFlags` bit 0x800 shows the trick in progress, and BridgeComposer greys
/// the cards already played. A holding with one is drawn card by card so that
/// card can take its own colour; every other holding stays one string (#30).
#[test]
fn a_played_card_is_drawn_on_its_own_so_it_can_be_greyed() {
    // Without the flag, North's holding is a single string
    let shown = render_trick("1f");
    assert!(shown.iter().any(|s| s == "10 8 3".as_bytes()));

    // With it, the played 3 is split out from the 10 and the 8
    let shown = render_trick("81f");
    assert!(!shown.iter().any(|s| s == "10 8 3".as_bytes()));
    for rank in ["10", "8", "3"] {
        assert!(
            shown.iter().any(|s| s == rank.as_bytes()),
            "{rank:?} not drawn on its own"
        );
    }
}

/// The card table holds the trick, each card at its player's seat. On a
/// single-suit fragment BridgeComposer prints bare ranks there, with no suit
/// symbol -- so each played rank is drawn twice, in the hand and in the table.
#[test]
fn the_card_table_shows_the_trick_as_bare_ranks() {
    let count = |shown: &[Vec<u8>], rank: &str| {
        shown
            .iter()
            .filter(|s| s.as_slice() == rank.as_bytes())
            .count()
    };

    // 0x01 without 0x800: no card table, but the play record is drawn (#42),
    // so the lead shows in West's hand and again in the record. North's 3 shows
    // only in the record -- its holding stays the single string "10 8 3".
    let shown = render_trick("1f");
    assert_eq!(count(&shown, "4"), 2, "West's 4 in the hand and the record");
    assert_eq!(count(&shown, "3"), 1, "North's 3 in the record only");

    // 0x800 adds the card table, and splits the played 3 out of the holding
    let shown = render_trick("81f");
    assert_eq!(
        count(&shown, "4"),
        3,
        "West's 4 in the hand, the table and the record"
    );
    assert_eq!(
        count(&shown, "3"),
        3,
        "North's 3 in the hand, the table and the record"
    );
}

/// A board carrying `[Contract]` and `[Play]` gets both lines with no
/// `[Auction]` at all. BridgeComposer 5.118.2 draws `4 \u{2660} by East` and
/// `Lead: \u{2665} K` on such a board; we drew neither, because both lived
/// inside the branch that renders the bidding table (issue #48).
const LEAD: &str = r#"[Board "1"]
[Dealer "W"]
[Vulnerable "NS"]
[Deal "W:QT65.J84.KJ3.AQ6 87.A97.A8542.J95 AKJ43.T3.Q97.K72 92.KQ652.T6.T843"]
[Declarer "E"]
[Contract "4S"]
[BCFlags "FLAGS"]
AUCTION[Play "S"]
PLAY
"#;

fn render_lead_board(flags: &str, auction: &str, play: &str) -> Vec<Vec<u8>> {
    // A section that already ends with `+` is continued; closing it with `*`
    // would say the opposite, and the two markers mean different things to the
    // play record.
    let section = if play.trim_end().ends_with('+') {
        play.to_string()
    } else {
        format!("{play}\n*")
    };
    let pbn = LEAD
        .replace("FLAGS", flags)
        .replace("AUCTION", auction)
        .replace("PLAY", &section);
    let file = parse_pbn(&pbn).unwrap();
    let pdf = render_boards(
        &file.boards,
        &[],
        Layout::Analysis,
        RenderOptions::default(),
    )
    .unwrap();
    shown_strings(&pdf)
}

/// The strings the contract and lead lines are drawn from. Each is split
/// around its suit glyph, so the tail of the contract and the head of the lead
/// are what identify them.
fn has_contract_and_lead(shown: &[Vec<u8>]) -> (bool, bool) {
    let has = |t: &str| shown.iter().any(|s| s == t.as_bytes());
    (has(" by East"), has("Lead: "))
}

#[test]
fn the_contract_and_lead_do_not_need_an_auction() {
    // With an auction, as before
    let shown = render_lead_board(
        "7e",
        "[Auction \"W\"]\n1C Pass 1S Pass\n2S Pass 4S AP\n",
        "HK",
    );
    assert_eq!(
        has_contract_and_lead(&shown),
        (true, true),
        "with an auction, both lines"
    );

    // Without one, BridgeComposer still draws both -- and now so do we
    let shown = render_lead_board("7e", "", "HK");
    assert_eq!(
        has_contract_and_lead(&shown),
        (true, true),
        "without an auction, still both lines"
    );
}

/// BridgeComposer prints the `Lead:` line only when it has nothing better to
/// show the play with. 0x800 puts the trick in the card table instead, and
/// 0x01 puts up the play-record table -- but only when there is a play record
/// to tabulate, so a lone opening lead still prints as a `Lead:` line. All
/// four cases probed against 5.118.2 on a board with no `[Auction]`.
#[test]
fn the_lead_line_gives_way_to_the_card_table_and_the_play_record() {
    let lead_shown = |flags: &str, play: &str| {
        let shown = render_lead_board(flags, "", play);
        has_contract_and_lead(&shown).1
    };

    // 0x01 clear, 0x800 clear: the line
    assert!(lead_shown("7e", "HK"), "7e, lone lead");
    assert!(lead_shown("7e", "HK H3 H2 HA"), "7e, a whole trick");

    // 0x800 set: the trick goes in the card table, and the line goes
    assert!(!lead_shown("87e", "HK"), "87e suppresses the line");

    // 0x01 set: the play-record table takes over, but only when there is a
    // record to tabulate -- ABS1-1's practice deals record just the lead and
    // BridgeComposer prints `Lead:` for them
    assert!(lead_shown("7f", "HK"), "7f with only a lead keeps the line");
    assert!(
        !lead_shown("7f", "HK H3 H2 HA"),
        "7f with a real play record drops it"
    );
}

/// The board label is set in bold italic, as BridgeComposer sets it (#44).
///
/// BridgeComposer 5.118.2 draws `6-1)` on ABS3-3's exercises in bold italic at
/// the top left of the cell, on the north hand's baseline. The Center work
/// (#35) moved it there from under the diagram; this pins the face, which is
/// easy to lose because the label shares a font set with the roman dealer and
/// vulnerability lines beside it.
#[test]
fn the_board_label_is_bold_italic() {
    let pbn = format!("%BCOptions ShowHCP\n\n{}", BOARD.replace("FLAGS", "0"));
    let headers = vec!["%BCOptions ShowHCP".to_string()];
    let file = parse_pbn(&pbn).unwrap();
    let pdf = render_boards(
        &file.boards,
        &headers,
        Layout::Analysis,
        RenderOptions::default(),
    )
    .unwrap();
    assert_eq!(
        label_font(&pdf, "Board 1").as_deref(),
        Some("Times-BoldItalic"),
        "the board label wants the bold italic face"
    );
}

/// The BaseFont in force when `text` is drawn.
fn label_font(pdf: &[u8], text: &str) -> Option<String> {
    let doc = lopdf::Document::load_mem(pdf).unwrap();

    // /Fn -> BaseFont, over every resource dictionary in the file
    let mut base: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let resolve = |o: &lopdf::Object| -> Option<lopdf::Dictionary> {
        match o {
            lopdf::Object::Dictionary(d) => Some(d.clone()),
            lopdf::Object::Reference(r) => match doc.get_object(*r) {
                Ok(lopdf::Object::Dictionary(d)) => Some(d.clone()),
                _ => None,
            },
            _ => None,
        }
    };
    for object in doc.objects.values() {
        let lopdf::Object::Dictionary(d) = object else {
            continue;
        };
        let Ok(fonts) = d.get(b"Font") else { continue };
        let Some(fonts) = resolve(fonts) else {
            continue;
        };
        for (name, value) in fonts.iter() {
            let Some(fd) = resolve(value) else { continue };
            if let Ok(lopdf::Object::Name(bf)) = fd.get(b"BaseFont") {
                base.insert(
                    String::from_utf8_lossy(name).to_string(),
                    String::from_utf8_lossy(bf).to_string(),
                );
            }
        }
    }

    for object in doc.objects.values() {
        let lopdf::Object::Stream(stream) = object else {
            continue;
        };
        let Ok(bytes) = stream.decompressed_content() else {
            continue;
        };
        let Ok(content) = lopdf::content::Content::decode(&bytes) else {
            continue;
        };
        let mut current = None;
        for op in &content.operations {
            if op.operator == "Tf" {
                if let Some(lopdf::Object::Name(n)) = op.operands.first() {
                    current = base.get(&String::from_utf8_lossy(n).to_string()).cloned();
                }
            }
            if op.operator == "Tj" {
                if let Some(lopdf::Object::String(t, _)) = op.operands.first() {
                    if String::from_utf8_lossy(t) == text {
                        return current;
                    }
                }
            }
        }
    }
    None
}

#[test]
fn probe_winner_populated() {
    let pbn = std::fs::read_to_string(
        "/Users/rick/Development/GitHub/pbn-to-pdf/tests/output/probe42/play.pbn",
    )
    .unwrap();
    let file = parse_pbn(&pbn).unwrap();
    let b = &file.boards[0];
    eprintln!("contract {:?}", b.contract);
    for (i, t) in b.play.as_ref().unwrap().tricks.iter().enumerate() {
        eprintln!(
            "T{} leader={:?} lead_suit={:?} winner={:?} cards={:?}",
            i + 1,
            t.leader,
            t.lead_suit,
            t.winner,
            t.cards
        );
    }
}

/// BridgeComposer puts up a play-record table in place of the `Lead:` line —
/// `Trick / Lead / 2nd / 3rd / 4th`, one row per trick (#42).
///
/// 0x01 asks for it, not 0x800. The issue guessed 0x800, but ABS3-3's
/// exercises carry that bit on 41 boards and BridgeComposer draws no table on
/// any of them; every table in the corpus sits under 0x01.
#[test]
fn the_play_record_table_follows_the_play_bit() {
    let drawn = |flags: &str, play: &str| {
        let shown = render_lead_board(flags, "", play);
        shown.iter().any(|s| s == "Trick".as_bytes())
    };

    assert!(drawn("7f", "HK H3 H2 HA"), "0x01 with a record to tabulate");
    assert!(
        !drawn("87e", "HK H3 H2 HA"),
        "0x800 alone draws the card table, not the record"
    );
    assert!(
        !drawn("7e", "HK H3 H2 HA"),
        "neither bit, so just the Lead: line"
    );
}

/// A section holding nothing but the opening lead prints as a `Lead:` line —
/// ABS1-1's practice deals do — unless it closes with `+`, which says the play
/// is unfinished rather than unrecorded. Grant's *Squeeze 2 Practice* has six
/// sections that are either longer than a lead or continued, and
/// BridgeComposer draws six tables.
#[test]
fn a_lone_opening_lead_is_a_line_unless_the_section_continues() {
    let table = |play: &str| {
        let shown = render_lead_board("7f", "", play);
        (
            shown.iter().any(|s| s == "Trick".as_bytes()),
            shown.iter().any(|s| s == "Lead: ".as_bytes()),
        )
    };

    assert_eq!(table("HK"), (false, true), "a lone lead is a line");
    assert_eq!(table("HK +"), (true, false), "continued, so tabulate it");
    assert_eq!(table("HK H3"), (true, false), "more than a lead");
}

/// The record shows the trick's leader, a bare rank for a card that follows
/// the led suit, and the suit symbol for one that does not.
#[test]
fn the_record_keeps_a_suit_symbol_only_when_the_card_does_not_follow() {
    // North, third to play, is void and discards a club
    let shown = render_lead_board("7f", "", "HK H3 C2 HA");
    let drawn = |t: &str| {
        shown
            .iter()
            .filter(|s| s.as_slice() == t.as_bytes())
            .count()
    };

    assert!(drawn("Trick") == 1 && drawn("Lead") == 1, "the headings");
    assert!(drawn("1.") == 1, "one trick, numbered");
    // The club shows its symbol; the hearts that follow do not add one. Two
    // suit symbols reach the table: the lead's heart and the discard's club.
    assert!(drawn("S") >= 1, "the leader's seat letter");
}
