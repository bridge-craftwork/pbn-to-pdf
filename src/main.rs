use anyhow::{Context, Result};
use clap::Parser;
use std::fs;

use pbn_to_pdf::cli::{parse_board_range, Args, Layout};
use pbn_to_pdf::config::Settings;
use pbn_to_pdf::embed::embed;
use pbn_to_pdf::parser::parse_pbn;
use pbn_to_pdf::{render_with_settings, Board, EmbedSource};

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

    // Filter boards if range specified, keeping each board's place in the
    // file for the embedded payload
    let mut selected: Vec<(usize, Board)> = pbn_file.boards.into_iter().enumerate().collect();
    if let Some(ref range_spec) = args.boards {
        let allowed_boards = parse_board_range(range_spec)
            .map_err(|e| anyhow::anyhow!("Invalid board range: {}", e))?;
        selected.retain(|(_, b)| {
            b.number
                .map(|n| allowed_boards.contains(&n))
                .unwrap_or(false)
        });
    }
    let (positions, boards): (Vec<usize>, Vec<Board>) = selected.into_iter().unzip();

    if boards.is_empty() {
        anyhow::bail!("No boards to process");
    }

    log::info!("Processing {} boards", boards.len());

    // Build settings from CLI args and PBN metadata
    let settings = Settings::from_args(&args).with_metadata(&pbn_file.metadata);
    let layout = settings.layout;

    // Generate PDF
    let output_path = args.output_path();

    let (pdf_data, regions) = render_with_settings(&boards, &settings)
        .with_context(|| format!("Failed to generate {} PDF", layout.as_str()))?;
    let pdf_data = if args.no_embed {
        pdf_data
    } else {
        let name = args
            .input
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "source.pbn".to_string());
        let source = EmbedSource {
            name: &name,
            text: &pbn_content,
        };
        embed(pdf_data, &regions, &boards, &positions, &source, layout)
            .with_context(|| "Failed to embed the PDF's payload")?
            .pdf
    };

    // Write output
    fs::write(&output_path, pdf_data)
        .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;

    println!("Successfully wrote PDF to {}", output_path.display());

    Ok(())
}
