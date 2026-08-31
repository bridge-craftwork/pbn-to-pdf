# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

`pbn-to-pdf` is a Rust CLI tool that converts PBN (Portable Bridge Notation) files to PDF documents with Bridge Composer-style formatting. It produces professional-quality bridge hand diagrams suitable for teaching materials and publications.

See [README.md](README.md) for CLI usage, options, and examples.

## Build and Test Commands

**Use `./dev-build.sh` for local development builds, not bare cargo.** This repo depends on the sibling `bridge-types` crate as a git dependency, with a gitignored `[patch]` override in `.cargo/config.toml` redirecting it to the local checkout in `../bridge-types`. Cargo never lets a `[patch]` override an existing `Cargo.lock` pin, so bare `cargo build` silently compiles the GitHub revision instead of your local edits — and if the patch does take effect, it rewrites `Cargo.lock` with a local-path entry that must never be committed (CI has no sibling checkouts). The script keeps a separate local lock (`.cargo/dev.lock`), swaps it in around the cargo call, verifies the patched crate resolved to the local checkout, and leaves the committed `Cargo.lock` untouched. It accepts any cargo subcommand and arguments (`./dev-build.sh test`, `./dev-build.sh run -- file.pbn -o out.pdf`); with no arguments it runs `cargo build`.

For CI-parity builds (pre-commit checks, release verification) use `./dev-build.sh --ci test` (any cargo subcommand works after `--ci`) — it temporarily disables the local patches and builds with the committed lock's git pins. **Avoid bare cargo for anything that resolves dependencies** (build/test/check/run): with the patches present, a same-version patch is applied immediately and silently rewrites `Cargo.lock` to local-path entries, while a version mismatch makes the patches silently ignored — both wrong. The committed `Cargo.lock` must always pin `git+https://` sources for the internal crates; never commit a lock where those entries have lost their `source =` lines.

```bash
# Build the project
./dev-build.sh

# Build release version
./dev-build.sh build --release

# Run all tests
./dev-build.sh test

# Run integration tests only (generates PDFs in tests/output/)
./dev-build.sh test --test integration_test

# Run a specific integration test
./dev-build.sh test full_deck_compass --release

# Check for clippy warnings
./dev-build.sh clippy

# Format code (no dependency resolution; bare cargo is fine)
cargo fmt

# Run with a PBN file
./dev-build.sh run -- path/to/file.pbn -o output.pdf

# Build the WebAssembly package (see "WebAssembly" below)
./wasm-build.sh
```

## WebAssembly

The bindings live in their own crate, `wasm/` (`pbn-to-pdf-wasm`), which
path-depends on the renderer with `default-features = false`. It is a **separate
workspace**: `cargo test --workspace` at the root does not reach it, which is why
CI has a dedicated `WebAssembly` job.

`./wasm-build.sh` builds the package via `wasm-pack`. It exists because two
things are easy to get wrong and silent when you do:

- **`--cfg getrandom_backend="wasm_js"`.** `printpdf` and `lopdf` depend on
  `getrandom`, which has no entropy source on `wasm32-unknown-unknown`. The
  `wasm_js` feature (a `[target.'cfg(target_arch = "wasm32")'.dependencies]`
  entry in `Cargo.toml`) points it at `crypto.getRandomValues`, but getrandom
  0.3 also needs this cfg before it uses that backend. Without it the build
  succeeds and fails at runtime on the first render. It lives in the script
  rather than `.cargo/config.toml` because that file is gitignored.
- **Lockfile protection for a second workspace.** `wasm/` has its own
  `Cargo.lock`, and cargo config discovery walks *upward* — so it inherits the
  root `.cargo/config.toml` `[patch]` overrides and its lock is exposed to
  exactly the same silent rewrite the root one is. `./dev-build.sh --workspace
  wasm` protects it; `wasm-build.sh` goes through that. A dev build of `wasm/`
  without it commits a lock whose `bridge-types` entry has lost its `source =`
  line, which CI cannot resolve.

`dev-build.sh` grew two flags for this: `--exec` runs an arbitrary
cargo-invoking command inside the lockfile swap, and `--workspace <dir>` selects
which crate's lock is protected and runs the command there. It also warns when
the target lock is untracked, because that is the one case it cannot protect —
the first build of a new workspace writes a patched lock that looks ordinary.

### Feature layout

- `cli` (default) — clap and env_logger. Both binaries declare
  `required-features = ["cli"]`. `Settings::from_args` and the `Args` struct are
  gated on it; the shared enums (`Layout`, `PageSize`, ...) are always compiled
  and only their `ValueEnum` derive is conditional. The `wasm/` crate depends on
  this one with `default-features = false`, so it gets neither.

`Layout::as_str`/`FromStr` give non-clap consumers the same layout spellings the
CLI accepts; a test asserts they match clap's `ValueEnum` names so they can't
drift apart.

### Verifying a wasm build

`./wasm-build.sh --target nodejs --out-dir pkg-node && node wasm/verify.mjs`
renders every layout and validates the PDFs. Compiling is not sufficient
evidence that a wasm build works — the getrandom trap above and the SVG font
issue below both compile cleanly and fail (or silently degrade) only at runtime.

Every layout renders pixel-identical in native and wasm, verified by
rasterizing both and comparing. That depends on the card rank indices being
vector paths rather than `<text>` — see below.

## Web app

`web/` is a Vite + Vue 3 front-end over the wasm build, deployed to Cloudflare
Pages. It follows the layout used by Dealer3 and bridge-solver: `npm run wasm`
builds the bindings into `web/src/wasm/` (generated, gitignored), and the site
build consumes them from there — so the wasm must exist on disk before Vite runs.

A few things in `web/vite.config.js` are load-bearing rather than decorative:

- `base: './'` — relative asset URLs, so a build works from a subpath.
- `optimizeDeps.exclude` on the generated module — `wasm-pack --target web`
  loads the binary with `new URL('..._bg.wasm', import.meta.url)`, and dep
  optimisation would rewrite that URL in dev.
- `manualChunks` puts the engine in its own content-hashed chunk. It is ~21 MB
  and changes rarely; app code is ~30 kB gzipped. Without the split every
  app-code deploy re-downloads the engine for everyone.

The lesson library is the **Rotations** export of Baker Bridge, read from its
`manifest.json`, not the `bridge-classroom` one. Two reasons: the app export
carries interactive control directives that print onto a handout as "Make a
Plan, then click NEXT", and Rotations rotates each set for a seating. The
seating matters — `VIEW_FOR_LAYOUT` in `web/src/lib/baker.js` sends the
declarer's plans to the `South` rotation (the student always declares), bidding
sheets to `North-South` (declarer alternates between the partners), and the
dealer summary and analysis to `Full Table`. So choosing a layout chooses a
file, and one selection can span all three.

The layout gallery renders a thumbnail of each layout's first page *before*
anything is ticked, because recognising a layout is the point of showing it.
That is affordable only because previews render a handful of opening boards
rather than a lesson — see `Layout::preview_boards`. pdf.js rasterises page one;
the rest is dropped.

The engine is imported lazily (`web/src/lib/render.js`), so browsing the lesson
library and reading the page work while the wasm is still arriving. A failed
load is deliberately *not* cached, or one flaky network leaves the page
permanently broken.

The bindings select boards two ways, and the distinction is deliberate.
`renderPbn` honours `options.boards`, the CLI's `--boards` spec, which selects
by `[Board]` number. `renderFirstBoards` / `renderPreview` select
*positionally*: a sliced Baker Bridge set is renumbered from 1 per set, but an
arbitrary PBN may be numbered from anything, and a preview must not depend on
that.

`Layout::preview_boards` (in `src/cli/args.rs`, so the CLI shares it) says how
many boards make a representative first page. For the card-geometry layouts it
is a real capacity, and the integration test asserts it by rendering: that many
boards fill one page and one more spills. For `analysis` and `bidding-sheets` it
is a *sample*, because their paging follows commentary and auction length
respectively — consumers render the preview and show only its first page. All
six layouts preview in ~80 ms together, against ~740 ms for one full lesson.

Testing has three layers, because each misses what the others catch:
`npm test` (vitest) covers the pure logic, `node wasm/verify.mjs` covers the
renderer, and `npm run check:browser` drives the built site in a real Chromium
to prove they are wired together. The last one is not in CI — it needs a
browser on disk.

Cloudflare rather than GitHub Pages: builds land in seconds instead of the
5-10 minutes Pages can take, and it carries the `.com`/`.org` domains. Since it
is the *only* host, `pages.yml` fails loudly when the credential is missing
rather than skipping the deploy and going green.

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

## Reproducible output

Rendering the same input twice must produce the same bytes: Baker Bridge commits
its packaged PDFs, and non-determinism rewrites 184 files on every rebuild for no
content change. `rendering_is_byte_reproducible_across_runs` guards this for all
six layouts.

Anything that reaches the PDF in iteration order has to be ordered deliberately.
`CardAssets::load_faces` sorts before registering XObjects because a `HashSet`'s
iteration order is seeded per set (issue #11) — that was the whole of the bug,
and it showed only in the declarer's-plan layouts because they are the only ones
that embed card faces.

One caveat, and it is printpdf's rather than ours: each embedded font subset gets
a tag drawn from an RNG that resets per process and advances within one. So the
*Nth* render of a process always matches the Nth render of any other process —
a build doing the same sequence of renders is reproducible — but two renders of
the same input inside one process differ in that tag. That is why the test runs
the binary twice instead of calling `render_boards` twice.

## Card asset pipeline

`assets/cards/*.svg` are the 52 base cards; `assets/cards/variants/` holds the
24 reduced court-card assets derived from them. Two tools maintain these, and
they run in order:

```bash
python3 tools/svg_text_to_paths.py     # needs fonttools; usually already done
python3 tools/make_card_variants.py    # regenerate variants after any base edit
```

`svg_text_to_paths.py` is why the corner rank indices are `<path>` and not
`<text>`. The originals drew them as `<text font-family="Arial">`, which usvg
resolves through a *system* font database — so the same input rendered
differently on macOS, Linux and CI, and the glyphs vanished entirely in wasm
(`printpdf::Svg::parse` hardcodes an empty `usvg::Options` on `wasm32` and
exposes no way to supply fonts). Baking the outlines cost nothing in the PDF:
printpdf sets svg2pdf's `embed_text = false`, so those glyphs were already being
flattened to paths on every render. Measured on 2-, 4- and 8-board sets, the
PDFs got ~1% *smaller* and the rendering is visually unchanged.

The font is Arimo (SIL OFL), metrically identical to Arial — every glyph used
has a bit-identical advance width. `assets/fonts/Arimo-CardRanks.ttf` is a
14-glyph subset kept only so the tool can be re-run reproducibly; it is **not**
compiled into the binary. Its licence is `assets/fonts/LICENSE-Arimo.txt`.

Converted indices carry `class="rank-index"` (or `"rank-index mirror"`), which
is how `make_card_variants.py` picks them out — it classifies the other elements
by geometry, so the indices have to announce themselves.

## Embedded Assets

- **Fonts**: only a 5-glyph DejaVu Sans subset (the four suit symbols) is
  embedded, and it is the sole font program in the output PDFs. Body text uses
  the PDF standard-14 builtins (Times, Helvetica), which the viewer supplies --
  see `src/render/helpers/fonts.rs`.
- **Card SVGs**: 52 playing cards in `assets/cards/` (58.94mm × 85.61mm at 300 DPI)

## PBN Format Notes

**Section data on the tag line.** The standard puts a section's data on the
lines *after* its tag pair, but some producers write the first datum on the tag
line itself — `[Play "W"]SJ`. The parser accepts that and logs a warning. It
matters because the discarded datum is the whole of a Play section: every
opening lead in the Baker Bridge collection (6,656 of them) was silently
invisible, and a declarer's plan rendered with no lead box and no complaint.
Accepting is not endorsement — Baker-Bridge#42 tracks fixing the producer.



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
