//! `RenderOptions::all_play_records` (`--all-play-records`).
//!
//! BridgeComposer draws the play-record table only when `BCFlags` 0x01 asks.
//! Exports that record the play of every table -- Brill's, for one -- carry no
//! `BCFlags`, so by default they get just a `Lead:` line. The option draws the
//! table for every board whose play is worth tabulating.

use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};

/// One board with three tricks of play and no `BCFlags`.
const RECORD: &str = r#"[Event "E"]
[Site ""]
[Date ""]
[Board "1"]
[West ""]
[North ""]
[East ""]
[South ""]
[Dealer "N"]
[Vulnerable "None"]
[Deal "N:AQJ8.53.Q92.KQT2 96.J762.KT63.A83 KT72.AKQ8.874.J5 543.T94.AJ5.9764"]
[Declarer "N"]
[Contract "3NT"]
[Result "9"]
[Auction "N"]
1NT Pass 3NT AP
[Play "E"]
H2 H4 H5 HA
D3 DJ D2 DK
H6 H9 H3 HK
*
"#;

/// Every string the render draws.
fn drawn(options: RenderOptions) -> Vec<String> {
    let file = parse_pbn(RECORD).unwrap();
    let pdf = render_boards(&file.boards, &[], Layout::Analysis, options).unwrap();
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
        for op in content.operations.iter().filter(|op| op.operator == "Tj") {
            if let Some(lopdf::Object::String(text, _)) = op.operands.first() {
                words.push(String::from_utf8_lossy(text).into_owned());
            }
        }
    }
    words
}

#[test]
fn without_flags_the_play_is_a_lead_line() {
    let words = drawn(RenderOptions::default());
    assert!(words.iter().any(|w| w.starts_with("Lead")), "{words:?}");
    assert!(!words.iter().any(|w| w == "Trick"), "{words:?}");
}

#[test]
fn the_option_draws_the_table() {
    let words = drawn(RenderOptions {
        all_play_records: true,
        ..Default::default()
    });
    assert!(words.iter().any(|w| w == "Trick"), "{words:?}");
    assert!(
        !words.iter().any(|w| w.starts_with("Lead:")),
        "the table stands in the lead line's place: {words:?}"
    );
}
