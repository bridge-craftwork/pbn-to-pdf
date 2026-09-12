// Where the wasm engine's bytes go: the download, the module's sections, and
// linear memory at load and across a run of renders.
// Run after ./wasm-build.sh --target nodejs --out-dir pkg-node:
//
//   node wasm/measure-memory.mjs [file.pbn]    (default: tests/fixtures/Drury.pbn)
//
// Linear memory only ever grows, so the figure after the last render is the
// peak -- and it is what a browser tab holds for as long as the engine stays
// loaded. Downloads are in MB (10^6 bytes); memory and module sections are in
// MiB, since wasm allocates memory in 64 KiB pages.

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { basename, dirname, join } from "node:path";
import { gzipSync } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const pkg = join(here, "pkg-node");
const wasmPath = join(pkg, "pbn_to_pdf_wasm_bg.wasm");
const pbnPath = process.argv[2] ?? join(root, "tests", "fixtures", "Drury.pbn");

// The generated glue keeps the instance's exports in a module-local `wasm` and
// does not export its memory. Evaluate a copy that does, rather than patching
// the package on disk.
const gluePath = join(pkg, "pbn_to_pdf_wasm.js");
const glue = new Function(
  "require", "exports", "module", "__dirname", "__filename",
  readFileSync(gluePath, "utf8") + "\n;exports.__wasm = wasm;",
);
const mod = { exports: {} };
glue(createRequire(gluePath), mod.exports, mod, pkg, gluePath);
const engine = mod.exports;

const MB = (n) => (n / 1e6).toFixed(2) + " MB";
const MiB = (n) => (n / 1048576).toFixed(2) + " MiB";
const memory = () => engine.__wasm.memory.buffer.byteLength;

// Section sizes and the declared initial memory, read from the binary itself.
function sections(bytes) {
  const leb = (i) => { let r = 0, s = 0, b; do { b = bytes[i++]; r |= (b & 0x7f) << s; s += 7; } while (b & 0x80); return [r >>> 0, i]; };
  const out = { code: 0, data: 0, initialPages: 0 };
  for (let i = 8; i < bytes.length;) {
    const id = bytes[i++]; let len; [len, i] = leb(i);
    if (id === 10) out.code += len;
    if (id === 11) out.data += len;
    if (id === 5) { let j = i; [, j] = leb(j); j++; [out.initialPages] = leb(j); }
    i += len;
  }
  return out;
}

const bytes = readFileSync(wasmPath);
const s = sections(bytes);
console.log(`engine: ${basename(wasmPath)}`);
console.log(`  download   ${MB(bytes.length)} raw, ${MB(gzipSync(bytes, { level: 9 }).length)} gzipped`);
console.log(`  sections   code ${MiB(s.code)}, data ${MiB(s.data)} (static data, copied into memory at load)`);
console.log(`  memory     ${MiB(memory())} at load (${s.initialPages} pages declared)`);

const pbn = readFileSync(pbnPath, "utf8");
const boards = (pbn.match(/^\[Board /gm) || []).length;
console.log(`\nrenders of ${basename(pbnPath)} (${boards} boards): memory after each, and time`);
for (const pass of [1, 2]) {
  for (const layout of engine.layouts()) {
    const t = performance.now();
    const pdf = engine.renderPbn(pbn, layout);
    const ms = performance.now() - t;
    if (pass === 1) console.log(`  ${layout.padEnd(20)} ${MiB(memory()).padStart(10)}   ${ms.toFixed(0).padStart(4)} ms   pdf ${(pdf.length / 1024).toFixed(0)} KB`);
  }
  if (pass === 1) console.log(`  peak after one pass of every layout: ${MiB(memory())}`);
  else console.log(`  after a second pass: ${MiB(memory())} (no growth means no leak)`);
}
