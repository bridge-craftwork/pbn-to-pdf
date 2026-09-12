//! Commentary that stands inside an `[Auction]` or `[Play]` section.
//!
//! BridgeComposer 5.118.2 discards it: probed with a block between two auction
//! lines, between two play lines, and after the last auction line, and it
//! leaves out all three while drawing one that precedes `[Deal]`. A section's
//! data runs until the next tag, so a block standing in that run is section
//! data rather than commentary.
//!
//! The obvious alternative -- that these are the *auction commentary*, held
//! back by its own bit -- was tested and ruled out: `BCFlags ff`, which sets
//! 0x80 "Show the Auction Commentary", renders identically to `7f`.
//!
//! We match that by default. `--section-commentary`, or
//! `RenderOptions::section_commentary`, keeps it for the lesson sets whose
//! coaching prose is written there.

use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};

/// One board with a block after a later tag and another inside the auction.
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
[Result ""]
{FINALTEXT after a later tag}
[BCFlags "7f"]
[Auction "N"]
1NT Pass 3NT AP
{SECTIONTEXT inside the auction}
"#;

/// Every word the render draws.
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
fn a_block_inside_a_section_is_left_out() {
    let words = drawn(RenderOptions::default());
    assert!(
        words.iter().any(|w| w == "FINALTEXT"),
        "a block after a later tag still shows"
    );
    assert!(
        !words.iter().any(|w| w == "SECTIONTEXT"),
        "a block inside the auction is left out, as BridgeComposer leaves it"
    );
}

#[test]
fn section_commentary_keeps_it() {
    let words = drawn(RenderOptions {
        section_commentary: true,
        ..Default::default()
    });
    assert!(
        words.iter().any(|w| w == "SECTIONTEXT"),
        "asked for, so drawn"
    );
    assert!(words.iter().any(|w| w == "FINALTEXT"));
}
