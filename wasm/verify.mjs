// End-to-end check that the wasm build actually renders, not just compiles.
// Run after ./wasm-build.sh --target nodejs --out-dir pkg-node:
//
//   node wasm/verify.mjs
//
// Rendering exercises the whole stack in wasm: the nom parser, rustybuzz text
// shaping, the embedded fonts, svg2pdf on the card art, and lopdf compression
// (which is what needs a working getrandom backend).

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const { renderPbn, renderFirstBoards, renderPreview, previewBoardCount, boardCount, layouts, RenderOptions } =
  await import(
  join(here, "pkg-node", "pbn_to_pdf_wasm.js")
);

const outDir = join(root, "tests", "output");
mkdirSync(outDir, { recursive: true });

let failures = 0;
const check = (name, fn) => {
  try {
    fn();
    console.log(`  ok   ${name}`);
  } catch (e) {
    failures++;
    console.log(`  FAIL ${name}\n       ${e.message}`);
  }
};

const fixture = join(root, "tests", "fixtures", "Stayman.pbn");
const pbn = readFileSync(fixture, "utf8");

console.log(`layouts: ${layouts().join(", ")}`);
const boards = boardCount(pbn);
console.log(`boards in Stayman.pbn: ${boards}`);

check("boardCount finds boards", () => {
  if (boards < 1) throw new Error(`expected >= 1 board, got ${boards}`);
});

// Every layout must render a structurally valid PDF.
for (const layout of layouts()) {
  check(`renderPbn(${layout})`, () => {
    const pdf = renderPbn(pbn, layout);
    if (!(pdf instanceof Uint8Array)) throw new Error("not a Uint8Array");
    const header = Buffer.from(pdf.subarray(0, 5)).toString("latin1");
    if (header !== "%PDF-") throw new Error(`bad header ${JSON.stringify(header)}`);
    const tail = Buffer.from(pdf.subarray(-1024)).toString("latin1");
    if (!tail.includes("%%EOF")) throw new Error("missing %%EOF trailer");
    const pages = (Buffer.from(pdf).toString("latin1").match(/\/Type\s*\/Page[^s]/g) || []).length;
    if (pages < 1) throw new Error("no page objects");
    writeFileSync(join(outDir, `wasm_${layout}.pdf`), pdf);
    console.log(`       ${(pdf.length / 1024).toFixed(0)} KB, ${pages} page(s)`);
  });
}

// The circling options are the one place the JS-side struct crosses over.
check("renderPbn honours RenderOptions", () => {
  const opts = new RenderOptions();
  opts.circleSureWinners = true;
  opts.circleLengthWinners = true;
  const plain = renderPbn(pbn, "declarers-plan-1up");
  const circled = renderPbn(pbn, "declarers-plan-1up", opts);
  if (circled.length === plain.length)
    throw new Error("circling produced a byte-identical PDF");
});

// Board selection: the whole point is that a preview costs a fraction of a
// full render, so check the size actually drops as well as that it works.
const full = renderPbn(pbn, "declarers-plan-2up");
check("renderPreview is much smaller than the full set", () => {
  const prev = renderPreview(pbn, "declarers-plan-2up");
  if (prev.length >= full.length)
    throw new Error(`preview ${prev.length} not smaller than full ${full.length}`);
  console.log(
    `       ${(full.length / 1024) | 0} KB full -> ${(prev.length / 1024) | 0} KB preview`,
  );
});

// The counts the layouts actually ask for.
check("previewBoardCount matches the layout geometry", () => {
  const want = {
    analysis: 1,
    "bidding-sheets": 5,
    "declarers-plan-1up": 1,
    "declarers-plan-2up": 2,
    "declarers-plan": 4,
    "dealer-summary": 6,
  };
  for (const [layout, n] of Object.entries(want)) {
    const got = previewBoardCount(layout);
    if (got !== n) throw new Error(`${layout}: expected ${n}, got ${got}`);
  }
});

// Every layout must produce a usable preview, in a time worth doing before a
// click rather than after one.
check("every layout previews quickly", () => {
  const t0 = performance.now();
  for (const layout of layouts()) {
    const b = renderPreview(pbn, layout);
    const head = Buffer.from(b.subarray(0, 5)).toString("latin1");
    if (head !== "%PDF-") throw new Error(`${layout} produced no PDF`);
  }
  const ms = performance.now() - t0;
  console.log(`       all ${layouts().length} previews in ${ms.toFixed(0)} ms`);
  if (ms > 3000) throw new Error(`previews took ${ms.toFixed(0)} ms`);
});

check("renderFirstBoards rejects a count of zero", () => {
  try {
    renderFirstBoards(pbn, "analysis", 0);
  } catch {
    return;
  }
  throw new Error("did not throw");
});

// A count larger than the file must render what is there, not fail.
check("renderFirstBoards tolerates an oversized count", () => {
  const b = renderFirstBoards(pbn, "dealer-summary", 100000);
  if (Buffer.from(b.subarray(0, 5)).toString("latin1") !== "%PDF-")
    throw new Error("no PDF");
});

check("options.boards selects a subset", () => {
  const opts = new RenderOptions();
  opts.boards = "1-2";
  const some = renderPbn(pbn, "declarers-plan-2up", opts);
  if (some.length >= full.length) throw new Error("subset was not smaller");
});

check("an empty boards spec renders everything", () => {
  const opts = new RenderOptions();
  opts.boards = "";
  const all = renderPbn(pbn, "declarers-plan-2up", opts);
  // Object ordering is not stable between renders, so compare page count.
  const pages = (b) => (Buffer.from(b).toString("latin1").match(/\/Type\s*\/Page[^s]/g) || []).length;
  if (pages(all) !== pages(full)) throw new Error(`${pages(all)} pages vs ${pages(full)}`);
});

check("a malformed boards spec throws", () => {
  const opts = new RenderOptions();
  opts.boards = "not-a-range";
  try {
    renderPbn(pbn, "declarers-plan-2up", opts);
  } catch (e) {
    if (!/Invalid board range/.test(e.message)) throw new Error(`wrong message: ${e.message}`);
    return;
  }
  throw new Error("did not throw");
});

// Selection is by [Board] number, not position: a spec that matches nothing
// must say so rather than rendering an empty document.
check("a boards spec that matches nothing throws", () => {
  const opts = new RenderOptions();
  opts.boards = "9999";
  try {
    renderPbn(pbn, "declarers-plan-2up", opts);
  } catch (e) {
    if (!/No boards matched/.test(e.message)) throw new Error(`wrong message: ${e.message}`);
    return;
  }
  throw new Error("did not throw");
});

// renderFirstBoard must not care how the boards are numbered -- a sliced set
// can start anywhere, and a preview that depended on it would break silently.
check("renderPreview ignores the numbering", () => {
  const renumbered = pbn.replace(/\[Board "(\d+)"\]/g, (_, n) => `[Board "${Number(n) + 500}"]`);
  const prev = renderPreview(renumbered, "declarers-plan-2up");
  const header = Buffer.from(prev.subarray(0, 5)).toString("latin1");
  if (header !== "%PDF-") throw new Error("did not render");
});

// Errors must arrive as JS exceptions, not wasm traps.
check("unknown layout throws", () => {
  try {
    renderPbn(pbn, "no-such-layout");
  } catch (e) {
    if (!/unknown layout/.test(e.message)) throw new Error(`wrong message: ${e.message}`);
    return;
  }
  throw new Error("did not throw");
});

// Input with no recognisable boards must be an error, not a blank PDF.
check("boardless PBN throws", () => {
  try {
    renderPbn("not a pbn file at all", "analysis");
  } catch (e) {
    if (!/No boards to process/.test(e.message))
      throw new Error(`wrong message: ${e.message}`);
    return;
  }
  throw new Error("did not throw");
});

console.log(failures ? `\n${failures} failure(s)` : "\nall wasm smoke tests passed");
process.exit(failures ? 1 : 0);
