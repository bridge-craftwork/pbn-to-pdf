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
