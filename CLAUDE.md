# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

`pbn-to-pdf` is a Rust CLI tool that converts PBN (Portable Bridge Notation) files to PDF documents with Bridge Composer-style formatting. It produces professional-quality bridge hand diagrams suitable for teaching materials and publications.

See [README.md](README.md) for CLI usage, options, and examples.

## Build and Test Commands

**Use `./dev-build.sh` for local development builds, not bare cargo.** This repo depends on the sibling `bridge-types` crate as a git dependency, with a gitignored `[patch]` override in `.cargo/config.toml` redirecting it to the local checkout in `../bridge-types`. Cargo never lets a `[patch]` override an existing `Cargo.lock` pin, so bare `cargo build` silently compiles the GitHub revision instead of your local edits — and if the patch does take effect, it rewrites `Cargo.lock` with a local-path entry that must never be committed (CI has no sibling checkouts). The script keeps a separate local lock (`.cargo/dev.lock`), swaps it in around the cargo call, verifies the patched crate resolved to the local checkout, and leaves the committed `Cargo.lock` untouched. It accepts any cargo subcommand and arguments (`./dev-build.sh test`, `./dev-build.sh run -- file.pbn -o out.pdf`); with no arguments it runs `cargo build`.

Bare cargo is correct only when you *want* the committed lock's git pins — i.e. reproducing exactly what CI builds (pre-commit checks, release verification). The committed `Cargo.lock` must always pin `git+https://` sources for internal crates; never commit a lock where those entries have lost their `source =` lines.

```bash
# Build the project
cargo build

# Build release version
cargo build --release

# Run all tests
cargo test

# Run integration tests only (generates PDFs in tests/output/)
cargo test --test integration_test

# Run a specific integration test
cargo test full_deck_compass --release

# Check for clippy warnings
cargo clippy

# Format code
cargo fmt

# Run with a PBN file
cargo run -- path/to/file.pbn -o output.pdf
```

## Architecture

The codebase follows a layered architecture:

```
CLI (src/cli/) → Parser (src/parser/) → Model (src/model/) → Render (src/render/)
                                                                    ↓
                                                            Config (src/config/)
```

### Source Structure

```
src/
├── main.rs              # Entry point
├── lib.rs               # Library exports
├── error.rs             # Error types
├── cli/                 # Command-line argument parsing
├── config/              # Runtime settings from CLI and PBN metadata
├── parser/              # PBN file parsing (nom combinators)
│   ├── pbn.rs           # Main file parser
│   ├── deal.rs          # Hand distribution (N:AKQ.JT9.876.5432)
│   ├── auction.rs       # Bidding sequence parsing
│   ├── commentary.rs    # Formatted text with suit codes
│   └── header.rs        # Bridge Composer % directives
├── model/               # Data structures for bridge concepts
│   ├── card.rs          # Suit, Rank, Card with Unicode symbols
│   ├── hand.rs          # Holding, Hand with HCP calculation
│   ├── auction.rs       # Call, Auction, Contract
│   └── board.rs         # Complete game record
└── render/              # PDF generation using printpdf
    ├── layouts/         # Page layout orchestration
    │   ├── analysis.rs      # Standard hand analysis layout
    │   └── bidding_sheets.rs # Practice bidding sheets
    ├── components/      # Reusable rendering components
    │   ├── hand_diagram.rs  # Compass-rose hand display
    │   ├── bidding_table.rs # Auction in W/N/E/S columns
    │   ├── commentary.rs    # Justified text with floating
    │   ├── fan.rs           # Fan-style card display (held cards)
    │   └── dummy.rs         # Dummy-style card display (table layout)
    └── helpers/         # Low-level rendering utilities
        ├── fonts.rs         # Embedded fonts (DejaVu, TeX Gyre)
        ├── text_metrics.rs  # Text measurement with rustybuzz
        ├── layer.rs         # LayerBuilder for printpdf 0.8
        ├── card_assets.rs   # SVG card images as XObjects
        ├── colors.rs        # Color definitions
        └── layout.rs        # Layout calculations
```

### Key Concepts

- **Render hierarchy**: Layouts compose Components, which use Helpers
- **LayerBuilder**: Collects PDF operations for printpdf 0.8's new API
- **CardAssets**: Loads 52 SVG card images as reusable XObjects
- **FanRenderer/DummyRenderer**: Card display with accurate bounding boxes

## Tests

```
tests/
├── fixtures/            # Sample PBN files for testing
├── integration_test.rs  # Integration tests for renderers
└── output/              # Generated PDFs (gitignored)
```

Integration tests generate PDFs in `tests/output/` for visual verification:
- `dummy_test.pdf` - Dummy renderer test
- `fan_test.pdf` - Fan renderer test
- `full_deck_compass.pdf` - Full 52-card compass layout

## Embedded Assets

- **Fonts**: DejaVu Sans, TeX Gyre Termes (embedded for cross-platform consistency)
- **Card SVGs**: 52 playing cards in `assets/cards/` (58.94mm × 85.61mm at 300 DPI)

## PBN Format Notes

Key PBN elements the parser handles:
- Tag pairs: `[Name "Value"]`
- Deal notation: `N:AKQ.JT9.876.5432 ...` (Spades.Hearts.Diamonds.Clubs)
- Auction: `1D 1S X Pass 1NT AP`
- Commentary: `{<b>Bidding.</b> Open 1\D with this hand...}`
- Header directives: `%BCOptions Float Justify ShowHCP`

## Code Style

- Use `cargo fmt` for formatting
- Run `cargo clippy` before committing
- Prefer editing existing files over creating new ones
- Keep functions focused and reasonably sized
- Use descriptive variable names for bridge concepts
