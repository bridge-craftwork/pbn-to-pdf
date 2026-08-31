use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use pbn_to_pdf::config::Settings;
use pbn_to_pdf::model::analysis::{
    find_length_winners, find_promotable_winners, find_sure_winners,
};
use pbn_to_pdf::model::{BidSuit, Card, Direction, Hand, Holding, Rank, Suit};
use pbn_to_pdf::parser::parse_pbn;
use pbn_to_pdf::render::components::{
    DeclarersPlanSmallRenderer, DummyRenderer, FanRenderer, LosersTableRenderer,
    WinnersTableRenderer,
};
use pbn_to_pdf::render::generate_pdf;
use pbn_to_pdf::render::helpers::colors::{SuitColors, BLUE, RED};
use pbn_to_pdf::render::helpers::FontManager;
use pbn_to_pdf::render::helpers::{CardAssets, LayerBuilder};
use pbn_to_pdf::{render_boards, Layout, RenderOptions};
use printpdf::{Mm, PdfDocument, PdfPage, PdfSaveOptions, PdfWarnMsg};

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn test_two_column_layout() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Parse the exercises PBN file which has %BoardsPerPage fit,2
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length exercises.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");

    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // Verify that column_count was detected from metadata
    assert_eq!(
        pbn_file.metadata.layout.column_count, 2,
        "Expected column_count to be 2 from %BoardsPerPage fit,2"
    );

    // Create settings and verify column_count is set
    let settings = Settings::default().with_metadata(&pbn_file.metadata);
    assert_eq!(
        settings.column_count, 2,
        "Expected settings.column_count to be 2"
    );

    // Generate PDF
    let pdf_bytes = generate_pdf(&pbn_file.boards, &settings).expect("Failed to generate PDF");

    // PDF should be non-empty
    assert!(!pdf_bytes.is_empty());

    // PDF should start with %PDF header
    assert!(pdf_bytes.starts_with(b"%PDF"));

    // Should be a reasonable size (multiple boards fit on fewer pages)
    assert!(pdf_bytes.len() > 10_000);

    // Write to output for visual verification
    let output_file = output_dir.join("two_column_layout_test.pdf");
    fs::write(&output_file, &pdf_bytes).expect("Failed to write test PDF");
    println!("Two-column layout test PDF written to: {:?}", output_file);
}

#[test]
fn test_parse_abs2_practice_deals() {
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");

    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // Should have 4 boards
    assert_eq!(pbn_file.boards.len(), 4);

    // Check first board
    let board1 = &pbn_file.boards[0];
    assert_eq!(board1.number, Some(1));
    assert!(board1.dealer.is_some());
    assert!(board1.auction.is_some());

    // Check that all boards have deals
    for board in &pbn_file.boards {
        assert!(board.deal.north.card_count() > 0);
        assert!(board.deal.south.card_count() > 0);
        assert!(board.deal.east.card_count() > 0);
        assert!(board.deal.west.card_count() > 0);
    }
}

#[test]
fn test_generate_pdf_from_abs2() {
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");

    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");
    let settings = Settings::default().with_metadata(&pbn_file.metadata);

    let pdf_bytes = generate_pdf(&pbn_file.boards, &settings).expect("Failed to generate PDF");

    // PDF should be non-empty
    assert!(!pdf_bytes.is_empty());

    // PDF should start with %PDF header
    assert!(pdf_bytes.starts_with(b"%PDF"));

    // Should be a reasonable size (at least 10KB for 4 boards)
    assert!(pdf_bytes.len() > 10_000);
}

#[test]
fn test_board_metadata_extraction() {
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");

    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // Check metadata was extracted
    assert!(pbn_file.metadata.version.is_some());
    assert!(pbn_file.metadata.creator.is_some());

    // Check layout settings
    assert!(pbn_file.metadata.layout.boards_per_page.is_some());
}

#[test]
fn test_commentary_parsing() {
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");

    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // At least some boards should have commentary
    let boards_with_commentary = pbn_file
        .boards
        .iter()
        .filter(|b| !b.commentary.is_empty())
        .count();

    assert!(
        boards_with_commentary > 0,
        "Expected some boards to have commentary"
    );
}

#[test]
fn test_auction_parsing() {
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");

    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    for board in &pbn_file.boards {
        if let Some(ref auction) = board.auction {
            // Auctions should have at least some calls
            assert!(
                !auction.calls.is_empty(),
                "Board {} has empty auction",
                board.number.unwrap_or(0)
            );

            // Should be able to determine final contract
            let contract = auction.final_contract();
            assert!(
                contract.is_some() || auction.is_passed_out,
                "Board {} should have contract or be passed out",
                board.number.unwrap_or(0)
            );
        }
    }
}

fn output_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/output")
}

#[test]
fn test_generate_all_pdfs() {
    // Ensure output directory exists
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Get all PBN files in fixtures
    let fixtures = fixtures_path();
    let pbn_files: Vec<_> = fs::read_dir(&fixtures)
        .expect("Failed to read fixtures directory")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "pbn")
                .unwrap_or(false)
        })
        .collect();

    assert!(!pbn_files.is_empty(), "No PBN files found in fixtures");

    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("Failed to build project");
    assert!(build_status.success(), "Failed to build project");

    let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/release/pbn-to-pdf");

    for entry in pbn_files {
        let pbn_path = entry.path();
        let stem = pbn_path.file_stem().unwrap().to_string_lossy();

        // Check if this is an "exercises" file (only generate Analysis layout)
        let is_exercises = stem.to_lowercase().ends_with("exercises");

        // Generate analysis PDF
        let analysis_output = output_dir.join(format!("{} - Analysis.pdf", stem));
        let status = Command::new(&binary)
            .args([
                "--layout",
                "analysis",
                pbn_path.to_str().unwrap(),
                "-o",
                analysis_output.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run pbn-to-pdf for analysis");
        assert!(
            status.success(),
            "Failed to generate analysis PDF for {}",
            stem
        );
        assert!(
            analysis_output.exists(),
            "Analysis PDF not created for {}",
            stem
        );

        // Skip other layouts for exercises files
        if is_exercises {
            println!("Generated Analysis PDF for exercises file: {}", stem);
            continue;
        }

        // Generate bidding sheets PDF
        let bidding_output = output_dir.join(format!("{} - Bidding Sheets.pdf", stem));
        let status = Command::new(&binary)
            .args([
                "--layout",
                "bidding-sheets",
                pbn_path.to_str().unwrap(),
                "-o",
                bidding_output.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run pbn-to-pdf for bidding-sheets");
        assert!(
            status.success(),
            "Failed to generate bidding sheets PDF for {}",
            stem
        );
        assert!(
            bidding_output.exists(),
            "Bidding sheets PDF not created for {}",
            stem
        );

        // Generate declarer's plan PDF
        let declarers_output = output_dir.join(format!("{} - Declarers Plan.pdf", stem));
        let status = Command::new(&binary)
            .args([
                "--layout",
                "declarers-plan",
                pbn_path.to_str().unwrap(),
                "-o",
                declarers_output.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run pbn-to-pdf for declarers-plan");
        assert!(
            status.success(),
            "Failed to generate declarer's plan PDF for {}",
            stem
        );
        assert!(
            declarers_output.exists(),
            "Declarer's plan PDF not created for {}",
            stem
        );

        // Generate dealer summary PDF
        let dealer_summary_output = output_dir.join(format!("{} - Dealer Summary.pdf", stem));
        let status = Command::new(&binary)
            .args([
                "--layout",
                "dealer-summary",
                pbn_path.to_str().unwrap(),
                "-o",
                dealer_summary_output.to_str().unwrap(),
            ])
            .status()
            .expect("Failed to run pbn-to-pdf for dealer-summary");
        assert!(
            status.success(),
            "Failed to generate dealer summary PDF for {}",
            stem
        );
        assert!(
            dealer_summary_output.exists(),
            "Dealer summary PDF not created for {}",
            stem
        );

        println!("Generated all layout PDFs for: {}", stem);
    }
}

/// Create a test hand with known cards
fn create_test_hand() -> Hand {
    // AKQ of spades, KQJ2 of hearts, A of diamonds, QJT98 of clubs
    Hand::from_holdings(
        Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]),
        Holding::from_ranks([Rank::King, Rank::Queen, Rank::Jack, Rank::Two]),
        Holding::from_ranks([Rank::Ace]),
        Holding::from_ranks([Rank::Queen, Rank::Jack, Rank::Ten, Rank::Nine, Rank::Eight]),
    )
}

/// Create a hand with void suits for testing edge cases
fn create_hand_with_voids() -> Hand {
    // 7 spades, 0 hearts, 3 diamonds, 3 clubs
    Hand::from_holdings(
        Holding::from_ranks([
            Rank::Ace,
            Rank::King,
            Rank::Queen,
            Rank::Jack,
            Rank::Ten,
            Rank::Nine,
            Rank::Eight,
        ]),
        Holding::new(), // void in hearts
        Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]),
        Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen]),
    )
}

#[test]
fn test_dummy_renderer_generates_pdf() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Create test hand
    let hand = create_test_hand();

    // Create PDF document
    let mut doc = PdfDocument::new("Dummy Renderer Test");

    // Load card assets
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");

    // Create renderer and layer - spades first with alternating colors, 20% overlap
    let renderer = DummyRenderer::with_overlap(&card_assets, 0.5, 0.20).first_suit(Suit::Spades);
    let mut layer = LayerBuilder::new();

    // Render the hand
    let height = renderer.render(&mut layer, &hand, (Mm(50.0), Mm(250.0)));

    // Create page with the rendered content
    let page = PdfPage::new(Mm(210.0), Mm(297.0), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 1000, "PDF should have reasonable size");
    assert!(height > 0.0, "Rendered height should be positive");

    // Write to output for visual verification
    let output_path = output_dir.join("dummy_test.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!("Dummy renderer test PDF written to: {:?}", output_path);
}

#[test]
fn test_fan_renderer_generates_pdf() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Create test hand
    let hand = create_test_hand();

    // Create PDF document
    let mut doc = PdfDocument::new("Fan Renderer Test");

    // Load card assets
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");

    // Create renderer and layer - with 45 degree arc for natural hand appearance
    let renderer = FanRenderer::new(&card_assets, 0.4).arc(45.0);
    let mut layer = LayerBuilder::new();

    // Render the hand
    let width = renderer.render(&mut layer, &hand, (Mm(20.0), Mm(180.0)));

    // Create page with the rendered content
    let page = PdfPage::new(Mm(297.0), Mm(210.0), layer.into_ops()); // Landscape for fan
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 1000, "PDF should have reasonable size");
    assert!(width > 0.0, "Rendered width should be positive");

    // Write to output for visual verification
    let output_path = output_dir.join("fan_test.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!("Fan renderer test PDF written to: {:?}", output_path);
}

#[test]
fn test_circled_cards() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Create test hand
    let hand = create_test_hand();

    // Define cards to circle (highlight) with colors
    // Include adjacent clubs to test overlapping ellipses
    // Use red for some cards and blue for others to demonstrate color support
    let mut circled: HashMap<Card, printpdf::Rgb> = HashMap::new();
    circled.insert(Card::new(Suit::Spades, Rank::Ace), RED);
    circled.insert(Card::new(Suit::Hearts, Rank::King), RED);
    circled.insert(Card::new(Suit::Diamonds, Rank::Queen), BLUE);
    circled.insert(Card::new(Suit::Clubs, Rank::Jack), BLUE);
    circled.insert(Card::new(Suit::Clubs, Rank::Queen), RED);
    circled.insert(Card::new(Suit::Clubs, Rank::Ten), BLUE);

    // Create PDF document
    let mut doc = PdfDocument::new("Circled Cards Test");

    // Load card assets
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");

    // Create layer for both renderers
    let mut layer = LayerBuilder::new();

    // Test FanRenderer with circled cards
    let fan_renderer = FanRenderer::new(&card_assets, 0.4)
        .arc(45.0)
        .circled_cards(circled.clone());
    fan_renderer.render(&mut layer, &hand, (Mm(20.0), Mm(180.0)));

    // Test DummyRenderer with circled cards
    let dummy_renderer = DummyRenderer::new(&card_assets, 0.4).circled_cards(circled);
    dummy_renderer.render(&mut layer, &hand, (Mm(20.0), Mm(80.0)));

    // Create page with the rendered content
    let page = PdfPage::new(Mm(297.0), Mm(210.0), layer.into_ops()); // Landscape
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 1000, "PDF should have reasonable size");

    // Write to output for visual verification
    let output_path = output_dir.join("circled_cards_test.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!("Circled cards test PDF written to: {:?}", output_path);
}

#[test]
fn test_dummy_with_void_suits() {
    // Create test hand with voids
    let hand = create_hand_with_voids();

    // Create PDF document
    let mut doc = PdfDocument::new("Dummy Void Test");

    // Load card assets
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");

    // Create renderer and layer - clubs first with alternating colors
    let renderer = DummyRenderer::new(&card_assets, 0.5).first_suit(Suit::Clubs);
    let mut layer = LayerBuilder::new();

    // Render the hand (should handle void suit gracefully)
    let height = renderer.render(&mut layer, &hand, (Mm(50.0), Mm(250.0)));

    // Create page with the rendered content
    let page = PdfPage::new(Mm(210.0), Mm(297.0), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(
        height > 0.0,
        "Rendered height should be positive even with voids"
    );

    // Write to output for visual verification
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");
    let output_path = output_dir.join("dummy_void_test.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
}

#[test]
fn test_card_renderer_dimensions() {
    // Create PDF document just to load assets
    let mut doc = PdfDocument::new("Dimensions Test");
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");

    let hand = create_test_hand();

    // Test DummyRenderer dimensions
    let dummy_renderer = DummyRenderer::new(&card_assets, 0.5);
    let (dummy_width, dummy_height) = dummy_renderer.dimensions(&hand);
    assert!(dummy_width > 0.0, "Dummy width should be positive");
    assert!(dummy_height > 0.0, "Dummy height should be positive");

    // Test FanRenderer dimensions
    let fan_renderer = FanRenderer::new(&card_assets, 0.4);
    let (fan_width, fan_height) = fan_renderer.dimensions(&hand);
    assert!(fan_width > 0.0, "Fan width should be positive");
    assert!(fan_height > 0.0, "Fan height should be positive");

    // Fan should be wider than tall, dummy should be taller than wide
    assert!(
        fan_width > fan_height,
        "Fan layout should be wider than tall"
    );
    // Note: dummy may or may not be taller than wide depending on hand shape
}

/// Create a full deal (4 hands with all 52 cards distributed)
fn create_full_deal() -> (Hand, Hand, Hand, Hand) {
    // North: A-K of each suit (8 cards) + Q-J-T of spades (3 cards) + 9-8 of hearts (2 cards) = 13 cards
    let north = Hand::from_holdings(
        Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]),
        Holding::from_ranks([Rank::Ace, Rank::King, Rank::Nine, Rank::Eight]),
        Holding::from_ranks([Rank::Ace, Rank::King]),
        Holding::from_ranks([Rank::Ace, Rank::King]),
    );

    // East: 9-8-7 of spades, Q-J-T of hearts, Q-J-T-9-8 of diamonds, Q-J of clubs = 13 cards
    let east = Hand::from_holdings(
        Holding::from_ranks([Rank::Nine, Rank::Eight, Rank::Seven]),
        Holding::from_ranks([Rank::Queen, Rank::Jack, Rank::Ten]),
        Holding::from_ranks([Rank::Queen, Rank::Jack, Rank::Ten, Rank::Nine, Rank::Eight]),
        Holding::from_ranks([Rank::Queen, Rank::Jack]),
    );

    // South: 6-5-4 of spades, 7-6-5-4 of hearts, 7-6 of diamonds, T-9-8-7 of clubs = 13 cards
    let south = Hand::from_holdings(
        Holding::from_ranks([Rank::Six, Rank::Five, Rank::Four]),
        Holding::from_ranks([Rank::Seven, Rank::Six, Rank::Five, Rank::Four]),
        Holding::from_ranks([Rank::Seven, Rank::Six]),
        Holding::from_ranks([Rank::Ten, Rank::Nine, Rank::Eight, Rank::Seven]),
    );

    // West: 3-2 of spades, 3-2 of hearts, 5-4-3-2 of diamonds, 6-5-4-3-2 of clubs = 13 cards
    let west = Hand::from_holdings(
        Holding::from_ranks([Rank::Three, Rank::Two]),
        Holding::from_ranks([Rank::Three, Rank::Two]),
        Holding::from_ranks([Rank::Five, Rank::Four, Rank::Three, Rank::Two]),
        Holding::from_ranks([Rank::Six, Rank::Five, Rank::Four, Rank::Three, Rank::Two]),
    );

    (north, east, south, west)
}

#[test]
fn test_full_deck_compass_layout() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Create the four hands
    let (north, east, south, west) = create_full_deal();

    // Verify we have exactly 52 cards
    let total_cards =
        north.card_count() + east.card_count() + south.card_count() + west.card_count();
    assert_eq!(total_cards, 52, "Should have exactly 52 cards in the deal");

    // Create PDF document (landscape A4 for better layout)
    let mut doc = PdfDocument::new("Full Deck Compass Layout");

    // Load card assets
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");

    // Create layer
    let mut layer = LayerBuilder::new();

    // Page dimensions (landscape A4)
    let page_width = 297.0;
    let page_height = 210.0;
    let center_x = page_width / 2.0;
    let center_y = page_height / 2.0;

    let dummy_scale = 0.35;
    let fan_scale = 0.42; // 20% larger than dummy
    let arc = 30.0;

    // Card dimensions for positioning calculations (at fan scale)
    let card_width = 58.94 * fan_scale; // CARD_WIDTH_MM * scale
    let card_height = 85.61 * fan_scale; // CARD_HEIGHT_MM * scale

    // Suit symbol width is roughly 1/6 of card width
    let suit_symbol_width = card_width / 6.0;

    // North: Dummy style at top center
    let dummy_renderer = DummyRenderer::with_overlap(&card_assets, dummy_scale, 0.20)
        .first_suit(Suit::Spades)
        .show_bounds(true);
    let (dummy_width, _) = dummy_renderer.dimensions(&north);
    let north_x = center_x - dummy_width / 2.0;
    let north_y = page_height - 15.0; // Near top
    dummy_renderer.render(&mut layer, &north, (Mm(north_x), Mm(north_y)));

    // South: Fan style at bottom center (facing up, like holding cards)
    // Scale the south fan to match the dummy width
    let temp_south_renderer = FanRenderer::new(&card_assets, 1.0).arc(arc);
    let (temp_south_width, _) = temp_south_renderer.dimensions(&south);
    let south_scale = dummy_width / temp_south_width;
    let south_renderer = FanRenderer::new(&card_assets, south_scale)
        .arc(arc)
        .show_bounds(true);
    let (south_width, south_height) = south_renderer.dimensions(&south);
    let south_x = center_x - south_width / 2.0;
    let south_y = 15.0 + south_height + 4.0 * suit_symbol_width; // Move up by 4 suit symbol widths
    south_renderer.render(&mut layer, &south, (Mm(south_x), Mm(south_y)));

    // West: Fan style rotated 90° clockwise (-90° CCW), on the left
    // Origin is now the CENTER of the rotated fan
    let west_renderer = FanRenderer::new(&card_assets, fan_scale)
        .arc(arc)
        .rotation(-90.0)
        .show_bounds(true);
    let west_x = 10.0 + 2.0 * card_height;
    let west_y = center_y + 2.0 * suit_symbol_width; // Move up by 2 suit symbol widths
    west_renderer.render(&mut layer, &west, (Mm(west_x), Mm(west_y)));

    // East: Fan style rotated 90° counter-clockwise (90° CCW), on the right
    // Origin is now the CENTER of the rotated fan - same Y as West for alignment
    let east_renderer = FanRenderer::new(&card_assets, fan_scale)
        .arc(arc)
        .rotation(90.0)
        .show_bounds(true);
    let east_x = page_width - 10.0 - 2.0 * card_height;
    let east_y = center_y + 2.0 * suit_symbol_width; // Move up by 2 suit symbol widths
    east_renderer.render(&mut layer, &east, (Mm(east_x), Mm(east_y)));

    // Create page with the rendered content (landscape A4)
    let page = PdfPage::new(Mm(page_width), Mm(page_height), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 5000, "PDF should have reasonable size");

    // Write to output for visual verification
    let output_path = output_dir.join("full_deck_compass.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!(
        "Full deck compass layout test PDF written to: {:?}",
        output_path
    );
}

#[test]
fn test_losers_table_generates_pdf() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Create PDF document
    let mut doc = PdfDocument::new("Losers Table Test");

    // Load fonts using FontManager
    let fonts = FontManager::new(&mut doc).expect("Failed to load fonts");

    // Create renderer with default colors
    let colors = SuitColors::default();
    let renderer = LosersTableRenderer::new(
        fonts.serif.regular,
        fonts.serif.bold,
        fonts.symbol_font(), // DejaVu Sans for suit symbols
        colors,
    );

    // Create layer and render
    let mut layer = LayerBuilder::new();
    let height = renderer.render(&mut layer, (Mm(50.0), Mm(250.0)));

    // Create page with the rendered content
    let page = PdfPage::new(Mm(210.0), Mm(297.0), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 1000, "PDF should have reasonable size");
    assert!(height > 0.0, "Rendered height should be positive");

    // Check dimensions
    let (width, expected_height) = renderer.dimensions();
    assert!(width > 0.0, "Table width should be positive");
    assert!(expected_height > 0.0, "Table height should be positive");

    // Write to output for visual verification
    let output_path = output_dir.join("losers_table_test.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!("Losers table test PDF written to: {:?}", output_path);
}

#[test]
fn test_winners_table_generates_pdf() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Create PDF document
    let mut doc = PdfDocument::new("Winners Table Test");

    // Load fonts using FontManager
    let fonts = FontManager::new(&mut doc).expect("Failed to load fonts");

    // Create renderer with default colors
    let colors = SuitColors::default();
    let renderer = WinnersTableRenderer::new(
        fonts.serif.regular,
        fonts.serif.bold,
        fonts.symbol_font(), // DejaVu Sans for suit symbols
        colors,
    );

    // Create layer and render
    let mut layer = LayerBuilder::new();
    let height = renderer.render(&mut layer, (Mm(50.0), Mm(250.0)));

    // Create page with the rendered content
    let page = PdfPage::new(Mm(210.0), Mm(297.0), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 1000, "PDF should have reasonable size");
    assert!(height > 0.0, "Rendered height should be positive");

    // Check dimensions
    let (width, expected_height) = renderer.dimensions();
    assert!(width > 0.0, "Table width should be positive");
    assert!(expected_height > 0.0, "Table height should be positive");

    // Write to output for visual verification
    let output_path = output_dir.join("winners_table_test.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!("Winners table test PDF written to: {:?}", output_path);
}

#[test]
fn test_declarers_plan_small_generates_pdf() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Create test hands (North dummy, South declarer)
    let north = Hand::from_holdings(
        Holding::from_ranks([Rank::Ace, Rank::King, Rank::Queen, Rank::Jack]),
        Holding::from_ranks([Rank::King, Rank::Queen, Rank::Jack]),
        Holding::from_ranks([Rank::Ace, Rank::Queen]),
        Holding::from_ranks([Rank::King, Rank::Queen, Rank::Jack, Rank::Ten]),
    );

    let south = Hand::from_holdings(
        Holding::from_ranks([Rank::Ten, Rank::Nine, Rank::Eight]),
        Holding::from_ranks([Rank::Ace, Rank::Ten, Rank::Nine, Rank::Eight]),
        Holding::from_ranks([Rank::King, Rank::Jack, Rank::Ten]),
        Holding::from_ranks([Rank::Ace, Rank::Nine, Rank::Eight]),
    );

    // Create PDF document
    let mut doc = PdfDocument::new("Declarer's Plan Small Test");

    // Load card assets and fonts
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");
    let fonts = FontManager::new(&mut doc).expect("Failed to load fonts");

    // Create renderer with default colors
    let colors = SuitColors::default();
    let renderer = DeclarersPlanSmallRenderer::new(
        &card_assets,
        fonts.serif.regular,
        fonts.serif.bold,
        fonts.symbol_font(),
        colors.clone(),
    )
    .show_bounds(true);

    // Create layer
    let mut layer = LayerBuilder::new();

    // Test 1: Suit contract with opening lead (losers table) - Hearts
    let opening_lead = Some(Card::new(Suit::Hearts, Rank::King));
    let height1 = renderer.render_with_info(
        &mut layer,
        &north,
        &south,
        false, // Suit contract
        opening_lead,
        Some(1),               // Deal 1
        Some("4♥"),            // Contract
        Some(BidSuit::Hearts), // Trump suit
        (Mm(15.0), Mm(280.0)),
    );

    // Test 2: NT contract without opening lead (winners table) - top right
    let height2 = renderer.render_with_info(
        &mut layer,
        &north,
        &south,
        true,                   // NT contract
        None,                   // No opening lead
        Some(2),                // Deal 2
        Some("3NT"),            // Contract
        Some(BidSuit::NoTrump), // Trump suit (NT)
        (Mm(115.0), Mm(280.0)),
    );

    // Test 3: Suit contract with opening lead (losers table) - bottom left - Diamonds (red)
    let opening_lead3 = Some(Card::new(Suit::Spades, Rank::Ace));
    let height3 = renderer.render_with_info(
        &mut layer,
        &north,
        &south,
        false, // Suit contract
        opening_lead3,
        Some(3),                 // Deal 3
        Some("5♦"),              // Contract
        Some(BidSuit::Diamonds), // Trump suit (red)
        (Mm(15.0), Mm(140.0)),
    );

    // Test 4: NT contract with opening lead (winners table) - bottom right
    let opening_lead4 = Some(Card::new(Suit::Diamonds, Rank::Queen));
    let height4 = renderer.render_with_info(
        &mut layer,
        &north,
        &south,
        true, // NT contract
        opening_lead4,
        Some(4),                // Deal 4
        Some("1NT"),            // Contract
        Some(BidSuit::NoTrump), // Trump suit (NT)
        (Mm(115.0), Mm(140.0)),
    );

    // Create page with the rendered content
    let page = PdfPage::new(Mm(210.0), Mm(297.0), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 1000, "PDF should have reasonable size");
    assert!(
        height1 > 0.0,
        "Rendered height should be positive for deal 1"
    );
    assert!(
        height2 > 0.0,
        "Rendered height should be positive for deal 2"
    );
    assert!(
        height3 > 0.0,
        "Rendered height should be positive for deal 3"
    );
    assert!(
        height4 > 0.0,
        "Rendered height should be positive for deal 4"
    );

    // Write to output for visual verification
    let output_path = output_dir.join("declarers_plan_small_test.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!(
        "Declarer's plan small test PDF written to: {:?}",
        output_path
    );
}

/// Rotate a deal so that the declarer is always South.
/// Returns (dummy_hand, declarer_hand) where declarer is positioned as South.
fn rotate_deal_for_declarer(deal: &pbn_to_pdf::model::Deal, declarer: Direction) -> (Hand, Hand) {
    match declarer {
        Direction::South => (deal.north.clone(), deal.south.clone()),
        Direction::North => (deal.south.clone(), deal.north.clone()),
        Direction::East => (deal.west.clone(), deal.east.clone()),
        Direction::West => (deal.east.clone(), deal.west.clone()),
    }
}

/// Green color for sure winners
const GREEN: printpdf::Rgb = printpdf::Rgb {
    r: 0.0,
    g: 0.6,
    b: 0.0,
    icc_profile: None,
};

/// Orange color for spent cards (used to drive out higher honors)
const ORANGE: printpdf::Rgb = printpdf::Rgb {
    r: 1.0,
    g: 0.5,
    b: 0.0,
    icc_profile: None,
};

/// Blue color for promoted winners
const PROMO_BLUE: printpdf::Rgb = printpdf::Rgb {
    r: 0.0,
    g: 0.4,
    b: 0.8,
    icc_profile: None,
};

#[test]
fn test_declarers_plan_with_sure_winners() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Parse the PBN file
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");
    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // Create PDF document
    let mut doc = PdfDocument::new("Declarer's Plan with Sure Winners");

    // Load card assets and fonts
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");
    let fonts = FontManager::new(&mut doc).expect("Failed to load fonts");

    // Create layer
    let mut layer = LayerBuilder::new();

    // Page layout: 2x2 grid (same as declarers_plan layout)
    let page_width = 215.9; // Letter width in mm
    let page_height = 279.4; // Letter height in mm
    let margin = 12.7; // ~0.5 inch margin
    let quadrant_padding = 5.0;

    let content_width = page_width - 2.0 * margin;
    let content_height = page_height - 2.0 * margin;
    let half_width = content_width / 2.0;
    let half_height = content_height / 2.0;

    let center_x = margin + half_width;
    let center_y = margin + half_height;

    // Origins for each quadrant (top-left corner of each, with padding)
    let positions = [
        (margin + quadrant_padding, page_height - margin), // Top-left
        (center_x + quadrant_padding, page_height - margin), // Top-right
        (margin + quadrant_padding, center_y),             // Bottom-left
        (center_x + quadrant_padding, center_y),           // Bottom-right
    ];

    // Draw separator lines
    let separator_color = printpdf::Rgb::new(0.3, 0.3, 0.3, None);
    layer.set_outline_color(printpdf::Color::Rgb(separator_color));
    layer.set_outline_thickness(2.0);
    layer.add_line(
        Mm(center_x),
        Mm(margin),
        Mm(center_x),
        Mm(page_height - margin),
    );
    layer.add_line(
        Mm(margin),
        Mm(center_y),
        Mm(page_width - margin),
        Mm(center_y),
    );

    let colors = SuitColors::default();

    // Process each board (up to 4)
    for (i, board) in pbn_file.boards.iter().take(4).enumerate() {
        let (x, y) = positions[i];

        // Get declarer direction (default to South if not specified)
        let declarer = board
            .contract
            .as_ref()
            .map(|c| c.declarer)
            .unwrap_or(Direction::South);

        // Rotate deal so declarer is always South
        let (dummy_hand, declarer_hand) = rotate_deal_for_declarer(&board.deal, declarer);

        // Find sure winners by combining dummy and declarer hands
        let sure_winners = find_sure_winners(&dummy_hand, &declarer_hand);
        println!(
            "Board {}: Found {} sure winners",
            board.number.unwrap_or(0),
            sure_winners.len()
        );
        for card in &sure_winners {
            println!("  - {}", card);
        }

        // Convert sure winners to circled cards map with green color
        let circled_cards: HashMap<Card, printpdf::Rgb> =
            sure_winners.into_iter().map(|card| (card, GREEN)).collect();

        // Determine if NT contract
        let is_nt = board
            .contract
            .as_ref()
            .map(|c| c.suit == BidSuit::NoTrump)
            .unwrap_or(false);

        // Get opening lead if play sequence exists
        let opening_lead = board
            .play
            .as_ref()
            .and_then(|play| play.tricks.first().and_then(|trick| trick.cards[0]));

        // Format contract string
        let contract_str = board.contract.as_ref().map(|c| {
            let suit_symbol = match c.suit {
                BidSuit::Clubs => "♣",
                BidSuit::Diamonds => "♦",
                BidSuit::Hearts => "♥",
                BidSuit::Spades => "♠",
                BidSuit::NoTrump => "NT",
            };
            format!("{}{}", c.level, suit_symbol)
        });

        // Get trump suit for suit ordering
        let trump = board.contract.as_ref().map(|c| c.suit);

        // Create renderer with circled cards
        let renderer = DeclarersPlanSmallRenderer::new(
            &card_assets,
            fonts.serif.regular,
            fonts.serif.bold,
            fonts.symbol_font(),
            colors.clone(),
        )
        .circled_cards(circled_cards);

        renderer.render_with_info(
            &mut layer,
            &dummy_hand,
            &declarer_hand,
            is_nt,
            opening_lead,
            board.number,
            contract_str.as_deref(),
            trump,
            (Mm(x), Mm(y)),
        );
    }

    // Create page with the rendered content
    let page = PdfPage::new(Mm(page_width), Mm(page_height), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 5000, "PDF should have reasonable size");

    // Write to output for visual verification
    let output_path = output_dir.join("declarers_plan_sure_winners.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!(
        "Declarer's plan with sure winners PDF written to: {:?}",
        output_path
    );
}

#[test]
fn test_declarers_plan_with_promotable_winners() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Parse the PBN file
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");
    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // Create PDF document
    let mut doc = PdfDocument::new("Declarer's Plan with Promotable Winners");

    // Load card assets and fonts
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");
    let fonts = FontManager::new(&mut doc).expect("Failed to load fonts");

    // Create layer
    let mut layer = LayerBuilder::new();

    // Page layout: 2x2 grid (same as declarers_plan layout)
    let page_width = 215.9; // Letter width in mm
    let page_height = 279.4; // Letter height in mm
    let margin = 12.7; // ~0.5 inch margin
    let quadrant_padding = 5.0;

    let content_width = page_width - 2.0 * margin;
    let content_height = page_height - 2.0 * margin;
    let half_width = content_width / 2.0;
    let half_height = content_height / 2.0;

    let center_x = margin + half_width;
    let center_y = margin + half_height;

    // Origins for each quadrant (top-left corner of each, with padding)
    let positions = [
        (margin + quadrant_padding, page_height - margin), // Top-left
        (center_x + quadrant_padding, page_height - margin), // Top-right
        (margin + quadrant_padding, center_y),             // Bottom-left
        (center_x + quadrant_padding, center_y),           // Bottom-right
    ];

    // Draw separator lines
    let separator_color = printpdf::Rgb::new(0.3, 0.3, 0.3, None);
    layer.set_outline_color(printpdf::Color::Rgb(separator_color));
    layer.set_outline_thickness(2.0);
    layer.add_line(
        Mm(center_x),
        Mm(margin),
        Mm(center_x),
        Mm(page_height - margin),
    );
    layer.add_line(
        Mm(margin),
        Mm(center_y),
        Mm(page_width - margin),
        Mm(center_y),
    );

    let colors = SuitColors::default();

    // Process each board (up to 4)
    for (i, board) in pbn_file.boards.iter().take(4).enumerate() {
        let (x, y) = positions[i];

        // Get declarer direction (default to South if not specified)
        let declarer = board
            .contract
            .as_ref()
            .map(|c| c.declarer)
            .unwrap_or(Direction::South);

        // Rotate deal so declarer is always South
        let (dummy_hand, declarer_hand) = rotate_deal_for_declarer(&board.deal, declarer);

        // Find promotable winners by combining dummy and declarer hands
        let promotion_result = find_promotable_winners(&dummy_hand, &declarer_hand);
        println!(
            "Board {}: Found {} spent cards, {} promotable winners",
            board.number.unwrap_or(0),
            promotion_result.spent.len(),
            promotion_result.winners.len()
        );
        for card in &promotion_result.spent {
            println!("  Spent: {}", card);
        }
        for card in &promotion_result.winners {
            println!("  Winner: {}", card);
        }

        // Convert to circled cards map: orange for spent, blue for winners
        let mut circled_cards: HashMap<Card, printpdf::Rgb> = HashMap::new();
        for card in promotion_result.spent {
            circled_cards.insert(card, ORANGE);
        }
        for card in promotion_result.winners {
            circled_cards.insert(card, PROMO_BLUE);
        }

        // Determine if NT contract
        let is_nt = board
            .contract
            .as_ref()
            .map(|c| c.suit == BidSuit::NoTrump)
            .unwrap_or(false);

        // Get opening lead if play sequence exists
        let opening_lead = board
            .play
            .as_ref()
            .and_then(|play| play.tricks.first().and_then(|trick| trick.cards[0]));

        // Format contract string
        let contract_str = board.contract.as_ref().map(|c| {
            let suit_symbol = match c.suit {
                BidSuit::Clubs => "♣",
                BidSuit::Diamonds => "♦",
                BidSuit::Hearts => "♥",
                BidSuit::Spades => "♠",
                BidSuit::NoTrump => "NT",
            };
            format!("{}{}", c.level, suit_symbol)
        });

        // Get trump suit for suit ordering
        let trump = board.contract.as_ref().map(|c| c.suit);

        // Create renderer with circled cards
        let renderer = DeclarersPlanSmallRenderer::new(
            &card_assets,
            fonts.serif.regular,
            fonts.serif.bold,
            fonts.symbol_font(),
            colors.clone(),
        )
        .circled_cards(circled_cards);

        renderer.render_with_info(
            &mut layer,
            &dummy_hand,
            &declarer_hand,
            is_nt,
            opening_lead,
            board.number,
            contract_str.as_deref(),
            trump,
            (Mm(x), Mm(y)),
        );
    }

    // Create page with the rendered content
    let page = PdfPage::new(Mm(page_width), Mm(page_height), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 5000, "PDF should have reasonable size");

    // Write to output for visual verification
    let output_path = output_dir.join("declarers_plan_promotable_winners.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!(
        "Declarer's plan with promotable winners PDF written to: {:?}",
        output_path
    );
}

/// Purple color for duck cards (used to exhaust defenders)
const PURPLE: printpdf::Rgb = printpdf::Rgb {
    r: 0.6,
    g: 0.2,
    b: 0.8,
    icc_profile: None,
};

/// Cyan color for length winners
const CYAN: printpdf::Rgb = printpdf::Rgb {
    r: 0.0,
    g: 0.7,
    b: 0.7,
    icc_profile: None,
};

#[test]
fn test_declarers_plan_with_length_winners() {
    // Create output directory
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Parse the PBN file
    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");
    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // Create PDF document
    let mut doc = PdfDocument::new("Declarer's Plan with Length Winners");

    // Load card assets and fonts
    let card_assets = CardAssets::load(&mut doc).expect("Failed to load card assets");
    let fonts = FontManager::new(&mut doc).expect("Failed to load fonts");

    // Create layer
    let mut layer = LayerBuilder::new();

    // Page layout: 2x2 grid (same as declarers_plan layout)
    let page_width = 215.9; // Letter width in mm
    let page_height = 279.4; // Letter height in mm
    let margin = 12.7; // ~0.5 inch margin
    let quadrant_padding = 5.0;

    let content_width = page_width - 2.0 * margin;
    let content_height = page_height - 2.0 * margin;
    let half_width = content_width / 2.0;
    let half_height = content_height / 2.0;

    let center_x = margin + half_width;
    let center_y = margin + half_height;

    // Origins for each quadrant (top-left corner of each, with padding)
    let positions = [
        (margin + quadrant_padding, page_height - margin), // Top-left
        (center_x + quadrant_padding, page_height - margin), // Top-right
        (margin + quadrant_padding, center_y),             // Bottom-left
        (center_x + quadrant_padding, center_y),           // Bottom-right
    ];

    // Draw separator lines
    let separator_color = printpdf::Rgb::new(0.3, 0.3, 0.3, None);
    layer.set_outline_color(printpdf::Color::Rgb(separator_color));
    layer.set_outline_thickness(2.0);
    layer.add_line(
        Mm(center_x),
        Mm(margin),
        Mm(center_x),
        Mm(page_height - margin),
    );
    layer.add_line(
        Mm(margin),
        Mm(center_y),
        Mm(page_width - margin),
        Mm(center_y),
    );

    let colors = SuitColors::default();

    // Process each board (up to 4)
    for (i, board) in pbn_file.boards.iter().take(4).enumerate() {
        let (x, y) = positions[i];

        // Get declarer direction (default to South if not specified)
        let declarer = board
            .contract
            .as_ref()
            .map(|c| c.declarer)
            .unwrap_or(Direction::South);

        // Rotate deal so declarer is always South
        let (dummy_hand, declarer_hand) = rotate_deal_for_declarer(&board.deal, declarer);

        // Find length winners by combining dummy and declarer hands
        let length_result = find_length_winners(&dummy_hand, &declarer_hand);
        println!(
            "Board {}: Found {} duck cards, {} length winners",
            board.number.unwrap_or(0),
            length_result.ducks.len(),
            length_result.winners.len()
        );
        for card in &length_result.ducks {
            println!("  Duck: {}", card);
        }
        for card in &length_result.winners {
            println!("  Length winner: {}", card);
        }

        // Convert to circled cards map: purple for ducks, cyan for length winners
        let mut circled_cards: HashMap<Card, printpdf::Rgb> = HashMap::new();
        for card in length_result.ducks {
            circled_cards.insert(card, PURPLE);
        }
        for card in length_result.winners {
            circled_cards.insert(card, CYAN);
        }

        // Determine if NT contract
        let is_nt = board
            .contract
            .as_ref()
            .map(|c| c.suit == BidSuit::NoTrump)
            .unwrap_or(false);

        // Get opening lead if play sequence exists
        let opening_lead = board
            .play
            .as_ref()
            .and_then(|play| play.tricks.first().and_then(|trick| trick.cards[0]));

        // Format contract string
        let contract_str = board.contract.as_ref().map(|c| {
            let suit_symbol = match c.suit {
                BidSuit::Clubs => "♣",
                BidSuit::Diamonds => "♦",
                BidSuit::Hearts => "♥",
                BidSuit::Spades => "♠",
                BidSuit::NoTrump => "NT",
            };
            format!("{}{}", c.level, suit_symbol)
        });

        // Get trump suit for suit ordering
        let trump = board.contract.as_ref().map(|c| c.suit);

        // Create renderer with circled cards
        let renderer = DeclarersPlanSmallRenderer::new(
            &card_assets,
            fonts.serif.regular,
            fonts.serif.bold,
            fonts.symbol_font(),
            colors.clone(),
        )
        .circled_cards(circled_cards);

        renderer.render_with_info(
            &mut layer,
            &dummy_hand,
            &declarer_hand,
            is_nt,
            opening_lead,
            board.number,
            contract_str.as_deref(),
            trump,
            (Mm(x), Mm(y)),
        );
    }

    // Create page with the rendered content
    let page = PdfPage::new(Mm(page_width), Mm(page_height), layer.into_ops());
    let mut warnings: Vec<PdfWarnMsg> = Vec::new();
    let pdf_bytes = doc
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);

    // Verify PDF is valid
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "PDF should start with %PDF header"
    );
    assert!(pdf_bytes.len() > 5000, "PDF should have reasonable size");

    // Write to output for visual verification
    let output_path = output_dir.join("declarers_plan_length_winners.pdf");
    fs::write(&output_path, &pdf_bytes).expect("Failed to write test PDF");
    println!(
        "Declarer's plan with length winners PDF written to: {:?}",
        output_path
    );
}

#[test]
fn test_two_col_auctions_stayman_exercises() {
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let input_file = fixtures_path().join("ABS4-1 Stayman exercises.pbn");
    let pbn_content = fs::read_to_string(&input_file).expect("Failed to read PBN file");
    let pbn_file = parse_pbn(&pbn_content).expect("Failed to parse PBN file");

    // Verify that two_col_auctions was detected from metadata
    assert!(
        pbn_file.metadata.layout.two_col_auctions,
        "Expected two_col_auctions to be true from %BCOptions TwoColAuctions"
    );

    // Create settings and verify two_col_auctions is set
    let settings = Settings::default().with_metadata(&pbn_file.metadata);
    assert!(
        settings.two_col_auctions,
        "Expected settings.two_col_auctions to be true"
    );

    // Verify the first board with an auction has the blank call
    let board_1_1 = pbn_file
        .boards
        .iter()
        .find(|b| b.board_id.as_deref() == Some("1-1"))
        .expect("Board 1-1 not found");

    let auction = board_1_1
        .auction
        .as_ref()
        .expect("Board 1-1 should have auction");
    assert_eq!(
        auction.calls.len(),
        1,
        "Board 1-1 should have 1 call (blank)"
    );
    assert_eq!(
        auction.calls[0].call,
        pbn_to_pdf::model::Call::Blank,
        "Board 1-1's call should be Blank"
    );

    // Verify uncontested_pair detects N-S as bidding side
    let pair = auction.uncontested_pair();
    assert_eq!(
        pair,
        Some((Direction::North, Direction::South)),
        "Should detect N-S as bidding side for blank call"
    );

    // Generate PDF
    let pdf_bytes = generate_pdf(&pbn_file.boards, &settings).expect("Failed to generate PDF");

    // PDF should be non-empty and valid
    assert!(!pdf_bytes.is_empty());
    assert!(pdf_bytes.starts_with(b"%PDF"));

    // Write to output for visual verification
    let output_file = output_dir.join("stayman_exercises_test.pdf");
    fs::write(&output_file, &pdf_bytes).expect("Failed to write test PDF");
    println!("Stayman exercises PDF written to: {:?}", output_file);
}

/// Exercise the --circle-* CLI flags end-to-end through all three declarer's
/// plan layouts (4-up, 1-up, 2-up). Verifies the flags are wired from
/// Settings through the renderers and that the resulting PDFs are valid.
///
/// Visual output is written to tests/output/ for manual inspection.
#[test]
fn test_declarers_plan_circle_flags() {
    use pbn_to_pdf::render::{
        DeclarersPlan1UpRenderer, DeclarersPlan2UpRenderer, DeclarersPlanRenderer,
    };
    use pbn_to_pdf::Layout;

    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let pbn_path = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");
    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // Confirm at least one board has analysis hits, otherwise the test is
    // meaningless even if the PDF is valid.
    let any_sure_winners = pbn_file.boards.iter().any(|b| {
        !find_sure_winners(&b.deal.north, &b.deal.south).is_empty()
            || !find_sure_winners(&b.deal.south, &b.deal.north).is_empty()
    });
    assert!(
        any_sure_winners,
        "Fixture should contain at least one board with sure winners"
    );

    // All three circle flags enabled simultaneously
    let mut settings = Settings::for_layout(Layout::DeclarersPlan);
    settings.circle_sure_winners = true;
    settings.circle_promotable_winners = true;
    settings.circle_length_winners = true;

    // 4-up
    let renderer = DeclarersPlanRenderer::new(settings.clone());
    let pdf_4up = renderer
        .render(&pbn_file.boards)
        .expect("Failed to render 4-up PDF with circle flags");
    assert!(pdf_4up.starts_with(b"%PDF"));
    fs::write(output_dir.join("circle_flags_4up_test.pdf"), &pdf_4up)
        .expect("Failed to write 4-up test PDF");

    // 1-up
    let mut settings_1up = Settings::for_layout(Layout::DeclarersPlan1up);
    settings_1up.circle_sure_winners = true;
    settings_1up.circle_promotable_winners = true;
    settings_1up.circle_length_winners = true;
    let renderer = DeclarersPlan1UpRenderer::new(settings_1up);
    let pdf_1up = renderer
        .render(&pbn_file.boards)
        .expect("Failed to render 1-up PDF with circle flags");
    assert!(pdf_1up.starts_with(b"%PDF"));
    fs::write(output_dir.join("circle_flags_1up_test.pdf"), &pdf_1up)
        .expect("Failed to write 1-up test PDF");

    // 2-up
    let mut settings_2up = Settings::for_layout(Layout::DeclarersPlan2up);
    settings_2up.circle_sure_winners = true;
    settings_2up.circle_promotable_winners = true;
    settings_2up.circle_length_winners = true;
    let renderer = DeclarersPlan2UpRenderer::new(settings_2up);
    let pdf_2up = renderer
        .render(&pbn_file.boards)
        .expect("Failed to render 2-up PDF with circle flags");
    assert!(pdf_2up.starts_with(b"%PDF"));
    fs::write(output_dir.join("circle_flags_2up_test.pdf"), &pdf_2up)
        .expect("Failed to write 2-up test PDF");

    // Sanity check: with all flags enabled, the PDF should be larger than
    // the same layout with no flags (extra ellipse drawing ops). Compare
    // against a baseline render with no circles.
    let baseline_settings = Settings::for_layout(Layout::DeclarersPlan);
    let baseline = DeclarersPlanRenderer::new(baseline_settings)
        .render(&pbn_file.boards)
        .expect("Failed to render baseline 4-up PDF");
    assert!(
        pdf_4up.len() > baseline.len(),
        "PDF with circle flags ({} bytes) should be larger than baseline ({} bytes)",
        pdf_4up.len(),
        baseline.len()
    );
}

/// Verify that `<span style=color:#XXX>...</span>` tags in commentary text are
/// parsed into TextSpan::Colored and survive the full render pipeline.
///
/// The fixture's commentary includes:
///   `<i><span style=color:#00f>2 Over 1 Game Force</span></i>`
/// which should produce one italic-colored span (blue), not literal `<span>` text.
#[test]
fn test_colored_span_in_commentary() {
    use pbn_to_pdf::model::TextSpan;

    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let pbn_path = fixtures_path().join("Forcing 1NT - 3 - practice hands (5-8).pbn");
    let content = fs::read_to_string(&pbn_path).expect("Failed to read PBN file");
    let pbn_file = parse_pbn(&content).expect("Failed to parse PBN");

    // BridgeComposer omits the [Event] tag before the first board in some files.
    // The PBN parser handles this by auto-creating a board on a leading [Board] tag.
    assert_eq!(
        pbn_file.boards.len(),
        4,
        "Expected all 4 boards to be parsed even though file starts with [Board \"1\"]"
    );

    let mut found_blue_2over1 = false;
    for board in &pbn_file.boards {
        for block in &board.commentary {
            for span in &block.content.spans {
                if let TextSpan::Colored { text, italic, rgb } = span {
                    if text.contains("2 Over 1 Game Force") {
                        assert!(
                            *italic,
                            "Expected italic-colored span for '2 Over 1 Game Force'"
                        );
                        assert_eq!(
                            *rgb,
                            (0, 0, 255),
                            "Expected blue (#00f -> 0,0,255) for '2 Over 1 Game Force'"
                        );
                        found_blue_2over1 = true;
                    }
                }
            }
        }
    }
    assert!(
        found_blue_2over1,
        "Expected to find a blue italic-colored span containing '2 Over 1 Game Force'"
    );

    // Also verify that no literal '<span' text leaked through into any plain-text rendering
    for board in &pbn_file.boards {
        for block in &board.commentary {
            let plain = block.content.to_plain_text();
            assert!(
                !plain.contains("<span"),
                "Plain text should not contain raw <span tag: found in {:?}",
                plain
            );
        }
    }

    // End-to-end render via the public API
    let settings = Settings::default().with_metadata(&pbn_file.metadata);
    let pdf_bytes = generate_pdf(&pbn_file.boards, &settings).expect("Failed to generate PDF");
    assert!(pdf_bytes.starts_with(b"%PDF"));

    let output_file = output_dir.join("colored_span_test.pdf");
    fs::write(&output_file, &pdf_bytes).expect("Failed to write test PDF");
    println!("Colored span test PDF written to: {:?}", output_file);
}

/// The declarer's plan layouts must not embed the full court-card artwork for
/// cards they only ever show a sliver of.
///
/// Each of the twelve J/Q/K illustrations is 200KB-570KB in the PDF, and
/// printpdf writes every XObject a document owns whether a page draws it or
/// not. Registering all 52 full cards -- which is what a plain
/// `CardAssets::load` does -- produced a 3.9MB file for these four boards
/// regardless of content. The reduced band/corner variants plus selective
/// registration bring it under 300KB, so a regression here means either the
/// variants stopped being used or the whole deck is being registered again.
#[test]
fn test_declarers_plan_does_not_embed_unused_court_art() {
    let output_dir = output_path();
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let input = fixtures_path().join("ABS2-2 Promotion and Length practice deals.pbn");
    let content = fs::read_to_string(&input).expect("Failed to read fixture");
    let boards = parse_pbn(&content).expect("Failed to parse fixture");

    for (layout, name) in [
        (Layout::DeclarersPlan1up, "declarers_plan_size_1up.pdf"),
        (Layout::DeclarersPlan2up, "declarers_plan_size_2up.pdf"),
        (Layout::DeclarersPlan, "declarers_plan_size_4up.pdf"),
    ] {
        let bytes = render_boards(&boards.boards, &[], layout, RenderOptions::default())
            .expect("Failed to render");
        fs::write(output_dir.join(name), &bytes).expect("Failed to write PDF");

        // Generous headroom over the ~190KB these actually produce: this guards
        // against the whole deck coming back, not against small drift.
        assert!(
            bytes.len() < 600_000,
            "{name} is {} bytes; the full court-card artwork looks to be embedded again",
            bytes.len()
        );
        assert!(
            bytes.len() > 20_000,
            "{name} is only {} bytes; the card artwork looks to have gone missing",
            bytes.len()
        );
    }
}

/// `Layout::preview_boards` promises a first page that represents the layout.
///
/// For the card-geometry layouts that is checkable: exactly that many boards
/// fill one page, and one more spills. `analysis` and `bidding-sheets` are
/// excluded from that half because their paging is content-driven — commentary
/// length and auction length respectively — so their counts are samples rather
/// than capacities, and the preview shows the first page of what comes back.
#[test]
fn preview_boards_renders_a_representative_first_page() {
    let content = fs::read_to_string(fixtures_path().join("Stayman.pbn")).unwrap();
    let pbn = parse_pbn(&content).unwrap();
    let comments: Vec<String> = content
        .lines()
        .filter(|l| l.starts_with('%'))
        .map(String::from)
        .collect();

    let page_count = |boards: &[pbn_to_pdf::Board], layout: Layout| -> usize {
        let pdf = render_boards(boards, &comments, layout, RenderOptions::default()).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        // /Type /Page, but not the /Type /Pages node that holds them -- a plain
        // substring count of the former silently includes every one of the latter.
        text.match_indices("/Type")
            .filter(|(i, _)| {
                let rest = text[i + "/Type".len()..].trim_start();
                rest.strip_prefix("/Page")
                    .is_some_and(|after| !after.starts_with('s'))
            })
            .count()
    };

    // Boards that actually carry a deal; the fixture has placeholder entries
    // whose hands parse to nothing.
    let usable: Vec<pbn_to_pdf::Board> = pbn
        .boards
        .iter()
        .filter(|b| b.deal.north.card_count() == 13)
        .take(12)
        .cloned()
        .collect();
    assert!(
        usable.len() >= 8,
        "fixture no longer has enough usable boards"
    );

    for layout in Layout::ALL {
        let n = layout.preview_boards() as usize;
        assert!(n >= 1, "{layout} previews zero boards");
        assert!(
            n <= usable.len(),
            "{layout} previews more boards than the fixture provides"
        );

        let preview = page_count(&usable[..n], layout);
        assert!(preview >= 1, "{layout} preview produced no pages");

        // Only the card-geometry layouts have a capacity worth asserting. How
        // many boards fit a page of `analysis` depends on how much commentary
        // each carries, and a page of `bidding-sheets` on how long the auctions
        // run -- for those two the count is a sample, and the preview shows the
        // first page of whatever comes back.
        let fixed_geometry = matches!(
            layout,
            Layout::DeclarersPlan
                | Layout::DeclarersPlan1up
                | Layout::DeclarersPlan2up
                | Layout::DealerSummary
        );
        if fixed_geometry {
            assert_eq!(preview, 1, "{layout} preview should be a single page");
            let spilled = page_count(&usable[..n + 1], layout);
            assert!(
                spilled > preview,
                "{layout} fits more than preview_boards() = {n}; the count is too low",
            );
        }
    }
}
