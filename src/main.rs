use anyhow::{Context, Result};
use clap::Parser;
use std::fs;

use pbn_to_pdf::cli::{parse_board_range, Args, Layout};
use pbn_to_pdf::config::Settings;
use pbn_to_pdf::parser::parse_pbn;
use pbn_to_pdf::render::{
    generate_pdf, BiddingSheetsRenderer, DealerSummaryRenderer, DeclarersPlan1UpRenderer,
    DeclarersPlan2UpRenderer, DeclarersPlanRenderer, HandRecordRenderer,
};

fn main() -> Result<()> {
    let mut args = Args::parse();

    // Initialize logging
    env_logger::Builder::new()
        .filter_level(match args.verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            _ => log::LevelFilter::Debug,
        })
        .init();

    // Read input file
    let pbn_content = fs::read_to_string(&args.input)
        .with_context(|| format!("Failed to read input file: {}", args.input.display()))?;

    // Parse PBN
    let pbn_file = parse_pbn(&pbn_content).with_context(|| "Failed to parse PBN content")?;

    log::info!("Parsed {} boards from PBN file", pbn_file.boards.len());

    // No --layout: the file may ask for BridgeComposer's hand record
    if args.layout.is_none() {
        args.layout = Some(Layout::default_for(&pbn_file.metadata));
    }

    // Filter boards if range specified
    let boards = if let Some(ref range_spec) = args.boards {
        let allowed_boards = parse_board_range(range_spec)
            .map_err(|e| anyhow::anyhow!("Invalid board range: {}", e))?;

        pbn_file
            .boards
            .into_iter()
            .filter(|b| {
                b.number
                    .map(|n| allowed_boards.contains(&n))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        pbn_file.boards
    };

    if boards.is_empty() {
        anyhow::bail!("No boards to process");
    }

    log::info!("Processing {} boards", boards.len());

    // Build settings from CLI args and PBN metadata
    let settings = Settings::from_args(&args).with_metadata(&pbn_file.metadata);

    // Generate PDF
    let output_path = args.output_path();

    let pdf_data = match settings.layout {
        Layout::Analysis => {
            generate_pdf(&boards, &settings).with_context(|| "Failed to generate PDF")?
        }
        Layout::BiddingSheets => {
            let renderer = BiddingSheetsRenderer::new(settings);
            renderer
                .render(&boards)
                .with_context(|| "Failed to generate bidding sheets PDF")?
        }
        Layout::DeclarersPlan1up => {
            let renderer = DeclarersPlan1UpRenderer::new(settings);
            renderer
                .render(&boards)
                .with_context(|| "Failed to generate declarer's plan 1-up PDF")?
        }
        Layout::DeclarersPlan2up => {
            let renderer = DeclarersPlan2UpRenderer::new(settings);
            renderer
                .render(&boards)
                .with_context(|| "Failed to generate declarer's plan 2-up PDF")?
        }
        Layout::DeclarersPlan => {
            let renderer = DeclarersPlanRenderer::new(settings);
            renderer
                .render(&boards)
                .with_context(|| "Failed to generate declarer's plan PDF")?
        }
        Layout::DealerSummary => {
            let renderer = DealerSummaryRenderer::new(settings);
            renderer
                .render(&boards)
                .with_context(|| "Failed to generate dealer summary PDF")?
        }
        Layout::HandRecord => HandRecordRenderer::new(settings)
            .render(&boards)
            .with_context(|| "Failed to generate hand record PDF")?,
    };

    // Write output
    fs::write(&output_path, pdf_data)
        .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;

    println!("Successfully wrote PDF to {}", output_path.display());

    Ok(())
}
