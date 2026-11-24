// The renderer, loaded on demand.
//
// The wasm module is large (about 7.4 MB gzipped), so it is
// imported at first use rather than at page load: choosing a file, browsing the
// library and reading the page all work while it is still arriving.

let enginePromise = null

/// Resolves to the wasm module, starting the download on the first call.
export function loadEngine() {
  if (!enginePromise) {
    enginePromise = import('@/wasm/pbn_to_pdf_wasm.js').then(async (mod) => {
      await mod.default()
      return mod
    })
    // A failed load must not be cached, or every later attempt replays the
    // rejection and the page looks permanently broken after one flaky network.
    enginePromise.catch(() => {
      enginePromise = null
    })
  }
  return enginePromise
}

export async function layouts() {
  return (await loadEngine()).layouts()
}

// `embed` is { sourceName } for a PDF that will be kept, so it carries the
// Contract 5 payload (the source PBN and where each board is, for lesson-stage
// and other readers) under that name; null for a preview, which is only shown.
function buildOptions(engine, options, embed) {
  const opts = new engine.RenderOptions()
  opts.circleSureWinners = !!options?.circleSureWinners
  opts.circlePromotableWinners = !!options?.circlePromotableWinners
  opts.circleLengthWinners = !!options?.circleLengthWinners
  opts.allPlayRecords = !!options?.allPlayRecords
  opts.playBySeat = !!options?.playBySeat
  if (embed) opts.sourceName = embed.sourceName
  else opts.noEmbed = true
  return opts
}

/// -> raw PDF bytes for the whole file. `sourceName` is the filename the
/// embedded PBN carries.
export async function renderBytes(pbn, layout, options, sourceName = 'source.pbn') {
  const engine = await loadEngine()
  // renderPbn is synchronous and CPU-bound: it blocks this thread until the PDF
  // is done. Yield first so a spinner the caller just set actually paints.
  await new Promise((r) => setTimeout(r, 0))
  return engine.renderPbn(pbn, layout, buildOptions(engine, options, { sourceName }))
}

/// -> { bytes, pages } for the whole file, where `pages[i]` is the page board
/// `i` of the file (record `i` of `sourceRecords`) was drawn on, or 0 if it
/// wasn't drawn.
export async function renderWithPages(pbn, layout, options, sourceName = 'source.pbn') {
  const engine = await loadEngine()
  await new Promise((r) => setTimeout(r, 0))
  const rendered = engine.renderPbnWithMap(pbn, layout, buildOptions(engine, options, { sourceName }))
  const bytes = rendered.pdf
  const map = JSON.parse(rendered.clickMap || '{"blocks":[]}')
  rendered.free()
  return { bytes, pages: pagesByBoard(map) }
}

/// The click map's blocks as a page per board. Its `board` is the board's
/// place among those rendered, from 1; a board drawn twice keeps its first page.
export function pagesByBoard(map) {
  const pages = []
  for (const block of map.blocks ?? []) {
    if (block.kind !== 'board' || !(block.board >= 1)) continue
    pages[block.board - 1] ||= block.page
  }
  return Array.from(pages, (p) => p ?? 0)
}

/// Each board's record in `pbn` as [start, end] string indices.
export async function sourceRecords(pbn) {
  const flat = (await loadEngine()).sourceRecords(pbn)
  const out = []
  for (let i = 0; i + 1 < flat.length; i += 2) out.push([flat[i], flat[i + 1]])
  return out
}

/// -> { blob, url, bytes } for a rendered PDF. Caller revokes the URL.
export async function render(pbn, layout, options, sourceName) {
  const bytes = await renderBytes(pbn, layout, options, sourceName)
  const blob = new Blob([bytes], { type: 'application/pdf' })
  return { blob, url: URL.createObjectURL(blob), bytes: bytes.length }
}

/// The opening boards of `pbn` through `layout` — enough to make the first page
/// representative, which is what the gallery shows. Cheap: a few boards instead
/// of a whole lesson.
export async function renderPreviewBytes(pbn, layout, options) {
  const engine = await loadEngine()
  await new Promise((r) => setTimeout(r, 0))
  return engine.renderPreview(pbn, layout, buildOptions(engine, options, null))
}

/// Layout ids are the CLI's own spellings; this is only for display.
export const layoutLabel = (id) =>
  ({
    analysis: 'Analysis',
    'bidding-sheets': 'Bidding sheets',
    'declarers-plan-1up': "Declarer's plan — 1 per page",
    'declarers-plan-2up': "Declarer's plan — 2 per page",
    'declarers-plan': "Declarer's plan — 4 per page",
    'dealer-summary': 'Dealer summary',
    'hand-record': 'Hand record',
  })[id] ?? id

export const usesCardArt = (id) => id.startsWith('declarers-plan')

/// The layouts that draw play records.
export const drawsPlay = (id) => id === 'analysis'

/// Gallery order: the layouts a teacher reaches for most, first.
export const LAYOUT_ORDER = [
  'declarers-plan-2up',
  'declarers-plan-1up',
  'declarers-plan',
  'bidding-sheets',
  'dealer-summary',
  'analysis',
  'hand-record',
]

export const orderLayouts = (ids) =>
  [...ids].sort((a, b) => {
    const ia = LAYOUT_ORDER.indexOf(a)
    const ib = LAYOUT_ORDER.indexOf(b)
    return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib)
  })
