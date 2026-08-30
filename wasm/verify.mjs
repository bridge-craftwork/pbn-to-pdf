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
const { renderPbn, boardCount, layouts, RenderOptions } = await import(
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
