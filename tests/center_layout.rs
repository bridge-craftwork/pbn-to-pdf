//! Center mode on multi-column pages (issue #24): each commentary block goes
//! where Bridge Composer 5.118.2 puts it, decided by where it stood among the
//! record's tags.

use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};

const HEADERS: [&str; 3] = [
    "%BCOptions Center GutterH GutterV Justify STBorder STShade TwoColAuctions",
    "%BoardsPerPage fit,2",
    r#"%Translate "Board %" "%)""#,
];

/// One board with a block in each of the four places a block can stand.
const RECORD: &str = r#"[Event "E"]
[Site ""]
[Date ""]
{EVENTTEXT before the board}
[Board "6-1"]
[West ""]
[North ""]
[East ""]
[South ""]
[Dealer "N"]
[Vulnerable "None"]
[SkillPath "x"]
{HIDDENTEXT between the board and the deal}
[Deal "N:AQJ8.53.Q92.KQT2 96.J762.KT63.A83 KT72.AKQ8.874.J5 543.T94.AJ5.9764"]
{DIAGRAMTEXT straight after the deal}
[Result ""]
{FINALTEXT after a later tag}
[BCFlags "FLAGS"]
[Auction "N"]
1C Pass 1H Pass
1S Pass 4S AP
"#;

/// Where every word is drawn -- (text, x, baseline) -- from the text
/// operators, with the multi-column Center headers.
fn words(flags: &str) -> Vec<(String, f32, f32)> {
    words_with(&HEADERS, flags)
}

fn words_with(headers: &[&str], flags: &str) -> Vec<(String, f32, f32)> {
    let pbn = format!(
        "{}\n\n{}",
        headers.join("\n"),
        RECORD.replace("FLAGS", flags)
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

    let mut words = Vec::new();
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
        let (mut x, mut y) = (0.0, 0.0);
        for op in &content.operations {
            match op.operator.as_str() {
                "Td" => {
                    x = op.operands[0].as_float().unwrap_or(0.0);
                    y = op.operands[1].as_float().unwrap_or(0.0);
                }
                "Tj" => {
                    if let Some(lopdf::Object::String(text, _)) = op.operands.first() {
                        words.push((String::from_utf8_lossy(text).into_owned(), x, y));
                    }
                }
                _ => {}
            }
        }
    }
    words
}

fn baseline(words: &[(String, f32, f32)], word: &str) -> Option<f32> {
    words.iter().find(|(w, _, _)| w == word).map(|&(_, _, y)| y)
}

fn left_edge(words: &[(String, f32, f32)], word: &str) -> Option<f32> {
    words.iter().find(|(w, _, _)| w == word).map(|&(_, x, _)| x)
}

#[test]
fn each_commentary_block_goes_where_bridge_composer_puts_it() {
    let words = words("7f");
    let y = |word: &str| baseline(&words, word).unwrap_or_else(|| panic!("{word} not drawn"));

    // Page coordinates grow upward, so each item sits below the one before
    let order = ["EVENTTEXT", "6-1)", "DIAGRAMTEXT", "North", "FINALTEXT"];
    for pair in order.windows(2) {
        assert!(
            y(pair[0]) > y(pair[1]),
            "{} should be above {}: {:?}",
            pair[0],
            pair[1],
            order.map(y)
        );
    }
    assert_eq!(
        baseline(&words, "HIDDENTEXT"),
        None,
        "between [Board] and [Deal]"
    );
}

#[test]
fn each_block_shows_under_its_own_bcflags_bit() {
    // 0x1f carries the final-commentary bit but not the event (0x20) or the
    // diagram-commentary (0x40) bits
    let words = words("1f");
    assert_eq!(baseline(&words, "EVENTTEXT"), None);
    assert_eq!(baseline(&words, "DIAGRAMTEXT"), None);
    assert!(baseline(&words, "FINALTEXT").is_some());
}

/// One board to a page (issue #25): the commentary no longer floats beside
/// the diagram, but runs full width below the board, as Bridge Composer's does.
#[test]
fn one_board_per_page_puts_commentary_below_the_board() {
    let headers = [
        "%BCOptions Center STBorder STShade",
        "%BoardsPerPage 1",
        r#"%Translate "Board %" "%)""#,
    ];
    let words = words_with(&headers, "7f");
    let y = |word: &str| baseline(&words, word).unwrap_or_else(|| panic!("{word} not drawn"));

    let order = ["EVENTTEXT", "6-1)", "DIAGRAMTEXT", "West", "FINALTEXT"];
    for pair in order.windows(2) {
        assert!(
            y(pair[0]) > y(pair[1]),
            "{} should be above {}: {:?}",
            pair[0],
            pair[1],
            order.map(y)
        );
    }
    assert_eq!(
        baseline(&words, "HIDDENTEXT"),
        None,
        "between [Board] and [Deal]"
    );
    // Full width from the left margin, where the board label starts, rather
    // than floated into the right half of the page
    assert_eq!(left_edge(&words, "FINALTEXT"), left_edge(&words, "6-1)"));
}
