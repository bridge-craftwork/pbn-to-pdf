//! `RenderOptions::play_by_seat` (`--play-by-seat`): the play record one
//! column per seat, with each card's cost from `[DoubleDummyPlay]`.

use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};

/// A board as Brill exports it, with bridge-solver's `--play` analysis.
/// East's ♦6 at trick three gave declarer a trick back.
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
[Deal "N:QJ7.8.T9752.J752 K542.T7.QJ6.Q943 A863.Q932.K84.K8 T9.AKJ654.A3.AT6"]
[Declarer "S"]
[Contract "3D"]
[Result "8"]
[Auction "N"]
Pass Pass 1D 1H
3D Pass Pass Pass
[Play "W"]
HA H8 H7 H2
ST SJ S5 S3
DA D9 D6 D4
S9 SQ SK SA
H4 D2 HT H9
D3 D5 DJ DK
H5 D7 DQ H3
HJ S7 S4 S8
HK DT C4 HQ
CA C2 C3 CK
H6 C5 C9 D8
C6 CJ CQ C8
CT C7 S2 S6
[DoubleDummyPlay "W"]
00.001111.11.111 * * *
00.11111.11.111 001... 1000... 1000...
..01. 01..00000.0000 ..001. ..200.
0.00000.0.111 01... * 011...
.10000.. 1..0000.3222 * 00.000.01.11
* 1..000.1111 * ..01.
* 1..00.1111 00..0.1000 00.00.1.11
.000..100 * 00...1111 *
* * 0...1000 0.0.2.00
...011 * ...100 *
* * 0...10 1..0.0
*+
"#;

/// Every string the render draws.
fn drawn(pbn: &str, options: RenderOptions) -> Vec<String> {
    let file = parse_pbn(pbn).unwrap();
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

fn by_seat() -> RenderOptions {
    RenderOptions {
        all_play_records: true,
        play_by_seat: true,
        ..Default::default()
    }
}

#[test]
fn bridge_composer_table_by_default() {
    let words = drawn(
        RECORD,
        RenderOptions {
            all_play_records: true,
            ..Default::default()
        },
    );
    assert!(words.iter().any(|w| w == "Trick"), "{words:?}");
    assert!(!words.iter().any(|w| w == "Declarer"), "{words:?}");
}

#[test]
fn columns_are_seats_from_the_opening_leader() {
    let words = drawn(RECORD, by_seat());
    assert!(!words.iter().any(|w| w == "Trick"), "{words:?}");
    // The compass rose draws W and E too, so read the five headings that
    // end with the score's
    let end = words.iter().position(|w| w == "Tricks").unwrap();
    assert_eq!(
        words[end - 4..=end],
        ["W", "Dummy", "E", "Declarer", "Tricks"]
    );
}

#[test]
fn costs_come_from_the_double_dummy_section() {
    let words = drawn(RECORD, by_seat());
    assert!(
        words
            .iter()
            .any(|w| w == "1 costed error, 1 trick given away"),
        "{words:?}"
    );
    // East is a defender, so the trick handed to declarer counts for them
    assert_eq!(
        words.iter().filter(|w| *w == "+1").count(),
        2,
        "badge and legend"
    );
    assert!(
        words.iter().any(|w| w == "declarer lost a trick"),
        "{words:?}"
    );
}

#[test]
fn without_the_section_there_are_no_costs() {
    let plain = RECORD.split("[DoubleDummyPlay").next().unwrap();
    let words = drawn(plain, by_seat());
    assert!(words.iter().any(|w| w == "Declarer"), "{words:?}");
    assert!(!words.iter().any(|w| w.contains("costed")), "{words:?}");
    assert!(!words.iter().any(|w| w == "+1" || w == "-1"), "{words:?}");
    assert!(words.iter().any(|w| w == "won the trick"), "{words:?}");
}
