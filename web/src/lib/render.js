// The renderer, loaded on demand.
//
// The wasm module is large (~21 MB, almost all of it card art), so it is
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

/// -> { blob, url, bytes } for a rendered PDF. Caller revokes the URL.
export async function render(pbn, layout, options) {
  const engine = await loadEngine()

  let opts
  if (options && Object.values(options).some(Boolean)) {
    opts = new engine.RenderOptions()
    opts.circleSureWinners = !!options.circleSureWinners
    opts.circlePromotableWinners = !!options.circlePromotableWinners
    opts.circleLengthWinners = !!options.circleLengthWinners
  }

  // Synchronous and CPU-bound: it blocks this thread until the PDF is done.
  // Yield first so a spinner queued by the caller actually paints.
  await new Promise((r) => setTimeout(r, 0))

  const bytes = engine.renderPbn(pbn, layout, opts)
  const blob = new Blob([bytes], { type: 'application/pdf' })
  return { blob, url: URL.createObjectURL(blob), bytes: bytes.length }
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
  })[id] ?? id

export const usesCardArt = (id) => id.startsWith('declarers-plan')
