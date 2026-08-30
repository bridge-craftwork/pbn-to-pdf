# pbn-to-pdf

A Rust CLI tool that converts PBN (Portable Bridge Notation) files to PDF with Bridge Composer-style formatting.

## Features

- Full table layout with 4 hands arranged around a compass rose
- Unicode suit symbols (♠♥♦♣) with red/black coloring
- Bidding table with West/North/East/South columns
- Commentary text with formatting (bold, italic, inline suit symbols)
- HCP (High Card Points) display for each hand
- Configurable page layout (1, 2, or 4 boards per page)
- Support for Letter, A4, and Legal paper sizes

## Installation

```bash
cargo build --release
```

The binary will be at `target/release/pbn-to-pdf`.

## Usage

```
pbn-to-pdf [OPTIONS] <INPUT>
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<INPUT>` | Input PBN file path (required) |

### Options

| Option | Description |
|--------|-------------|
| `-o, --output <OUTPUT>` | Output PDF file path (defaults to input with .pdf extension) |
| `-l, --layout <LAYOUT>` | Output layout style: analysis, bidding-sheets (default: analysis) |
| `-n, --boards-per-page <N>` | Number of boards per page: 1, 2, or 4 (default: 1) |
| `-s, --page-size <SIZE>` | Page size: letter, a4, legal (default: letter) |
| `--orientation <O>` | Page orientation: portrait, landscape (default: portrait) |
| `-m, --margins <PRESET>` | Page margins: narrow (1/4"), standard (1/2"), wide (1") |
| `--no-bidding` | Hide bidding table |
| `--no-play` | Hide play sequence |
| `--no-commentary` | Hide commentary text |
| `--no-hcp` | Hide HCP point counts |
| `-b, --boards <RANGE>` | Board range to include (e.g., "1-16" or "5,8,12") |
| `-t, --title [TITLE]` | Title for bidding sheets banner (overrides %HRTitleEvent; use with no value to hide) |
| `--debug-boxes` | Draw debug boxes around layout regions |
| `-v, --verbose` | Increase verbosity (-v, -vv, -vvv) |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### Examples

```bash
# Basic conversion (creates input.pdf)
pbn-to-pdf hands.pbn

# Specify output file
pbn-to-pdf hands.pbn -o output.pdf

# Two boards per page on A4 paper
pbn-to-pdf hands.pbn -n 2 -s a4

# Landscape orientation, 4 boards per page
pbn-to-pdf hands.pbn -n 4 --orientation landscape

# Only boards 1-8, no commentary
pbn-to-pdf hands.pbn -b 1-8 --no-commentary

# Specific boards only
pbn-to-pdf hands.pbn -b "1,5,9,13"

# Verbose output for debugging
pbn-to-pdf hands.pbn -vv

# Generate bidding practice sheets
pbn-to-pdf hands.pbn -l bidding-sheets -o practice.pdf

# Bidding sheets with wide margins
pbn-to-pdf hands.pbn -l bidding-sheets -m wide

# Bidding sheets with custom title
pbn-to-pdf hands.pbn -l bidding-sheets -t "My Practice Session"

# Bidding sheets with no title
pbn-to-pdf hands.pbn -l bidding-sheets -t
```

## Web app

There is a browser front-end in [`web/`](web/): choose a PBN — from the
[Baker Bridge](https://github.com/bridge-craftwork/Baker-Bridge) lesson library,
a local file, or a URL — pick a layout, and get a PDF. Rendering happens
entirely in the browser through the WebAssembly build, so nothing is uploaded.

```bash
cd web
npm install
npm run dev          # builds the wasm, then serves with hot reload
npm run build:all    # wasm + production build into web/dist
npm test             # unit tests
npm run check:browser  # end-to-end check against a running preview
```

Deployed to Cloudflare Pages by `.github/workflows/pages.yml`; the hosting
config is [`wrangler.jsonc`](wrangler.jsonc).

## WebAssembly

The renderer also builds for the browser and Node. `render_boards` is a pure
`boards -> PDF bytes` function and every asset (fonts, card art) is compiled in,
so the wasm build needs no filesystem and no network.

```bash
./wasm-build.sh                                  # bundler package in wasm/pkg/
./wasm-build.sh --target nodejs --out-dir pkg-node
./wasm-build.sh --target web                     # for a plain <script type="module">
```

The bindings are the `wasm/` crate (`pbn-to-pdf-wasm`), a separate workspace
that path-depends on the renderer.

```js
import init, { renderPbn, boardCount, layouts, RenderOptions } from "./pkg/pbn_to_pdf_wasm.js";

await init();

const pdf = renderPbn(pbnText, "declarers-plan-2up");   // Uint8Array of PDF bytes
const url = URL.createObjectURL(new Blob([pdf], { type: "application/pdf" }));
```

`layouts()` returns the layout names, which are the same strings the CLI's
`--layout` accepts. `RenderOptions` carries the declarer's-plan card-circling
flags. Errors (an unknown layout, a PBN with no boards) are thrown as JS
exceptions.

After building for Node, `node wasm/verify.mjs` renders every layout and checks
the resulting PDFs.

The bundle is large — about 21 MB raw, 8.8 MB gzipped — because the 52 card SVGs
are compiled in. Serve it compressed, and expect the fetch to dominate the first
render.

Output matches the native build: every layout renders pixel-identical PDFs in
both, verified by rasterizing and comparing.

## PBN Format Support

The tool supports PBN 2.1 format including:

- Standard tags: `[Event]`, `[Board]`, `[Dealer]`, `[Vulnerable]`, `[Deal]`, etc.
- Auction section with bids, doubles, redoubles, and "AP" (All Pass)
- Play section with card notation
- Commentary in braces `{...}` with formatting:
  - `<b>Bold text</b>`
  - `<i>Italic text</i>`
  - `\S` `\H` `\D` `\C` for suit symbols
  - `\SQ` `\HA` etc. for card references
- Bridge Composer header directives (`%BoardsPerPage`, `%Margins`, `%PipColors`, etc.)

## License

The source is released under the Unlicense (public domain).

Bundled assets come from third parties and are listed in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). In short: the card artwork is
public domain (Byron Knoll's *vector-playing-cards*); the embedded suit-symbol
font is a DejaVu Sans subset under the Bitstream Vera Fonts Copyright, which
requires its notice to travel with copies; and Arimo (SIL OFL), used only by the
asset tooling, ships with the repo but not in the binary.
