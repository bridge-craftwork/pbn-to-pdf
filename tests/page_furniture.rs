//! Page furniture (issue #28): the event header or heading and the
//! `%PageFooter` lines, printed as BridgeComposer 5.118.2 prints them unless
//! a pipeline asks for them to be left out.

use pbn_to_pdf::{parse_pbn, render_boards, Layout, RenderOptions};

const RECORD: &str = r#"[Event "Third-Hand Play"]
[Site ""]
[Date ""]
[Board "1"]
[Dealer "N"]
[Vulnerable "None"]
[Deal "N:AQJ8.53.Q92.KQT2 96.J762.KT63.A83 KT72.AKQ8.874.J5 543.T94.AJ5.9764"]
[Auction "N"]
1C Pass 1H Pass
1S Pass 4S AP
"#;

/// The footer ABS3-3 carries, plus a page count to find it by.
const FOOTERS: [&str; 4] = [
    "%HRTitleDate 2017.01.17",
    r#"%HRTitleSite "Stoneridge Creek""#,
    r#"%PageFooter:0,0 "%D\\n%s""#,
    r#"%PageFooter:0,2 "%n of %N""#,
];

/// Every string drawn, with its baseline.
fn drawn(headers: &[&str], omit_page_furniture: bool) -> Vec<(String, f32)> {
    let pbn = format!("{}\n\n{}", headers.join("\n"), RECORD);
    let file = parse_pbn(&pbn).unwrap();
    let headers: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let options = RenderOptions {
        omit_page_furniture,
        ..RenderOptions::default()
    };
    let pdf = render_boards(&file.boards, &headers, Layout::Analysis, options).unwrap();
    let doc = lopdf::Document::load_mem(&pdf).unwrap();

    let mut drawn = Vec::new();
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
        let mut y = 0.0;
        for op in &content.operations {
            match op.operator.as_str() {
                "Td" => y = op.operands[1].as_float().unwrap_or(0.0),
                "Tj" => {
                    if let Some(lopdf::Object::String(text, _)) = op.operands.first() {
                        drawn.push((String::from_utf8_lossy(text).into_owned(), y));
                    }
                }
                _ => {}
            }
        }
    }
    drawn
}

fn baseline(drawn: &[(String, f32)], text: &str) -> Option<f32> {
    drawn.iter().find(|(t, _)| t == text).map(|&(_, y)| y)
}

fn headers(extra: &[&'static str]) -> Vec<&'static str> {
    let mut all = vec!["%BoardsPerPage 1"];
    all.extend_from_slice(extra);
    all.extend_from_slice(&FOOTERS);
    all
}

#[test]
fn the_page_header_and_footers_print_by_default() {
    let drawn = drawn(&headers(&["%BCOptions Float Justify PageHeader"]), false);
    let y = |text: &str| baseline(&drawn, text).unwrap_or_else(|| panic!("{text:?} not drawn"));

    // The spelt-out date, the site on the cell's second line, and the page
    assert!(y("Tuesday, January 17, 2017") > y("Stoneridge Creek"));
    y("1 of 1");
    // The header sits above the board, in the top margin
    assert!(y("Third-Hand Play") > y("Board 1"));
}

#[test]
fn a_pipeline_can_leave_the_furniture_out() {
    let drawn = drawn(&headers(&["%BCOptions Float Justify PageHeader"]), true);
    for text in [
        "Third-Hand Play",
        "Tuesday, January 17, 2017",
        "Stoneridge Creek",
        "1 of 1",
    ] {
        assert_eq!(baseline(&drawn, text), None, "{text:?} drawn");
    }
    assert!(baseline(&drawn, "Board 1").is_some());
}

#[test]
fn without_page_header_the_event_heads_the_page_and_pushes_it_down() {
    let furnished = drawn(&headers(&["%BCOptions Center"]), false);
    let bare = drawn(&headers(&["%BCOptions Center"]), true);
    let heading = baseline(&furnished, "Third-Hand Play").expect("heading drawn");
    let label = baseline(&furnished, "Board 1").unwrap();
    assert!(heading > label);
    assert!(
        label < baseline(&bare, "Board 1").unwrap(),
        "board not pushed down"
    );
}
