// Turn the first page of a rendered PDF into an image for the layout gallery.
//
// The gallery exists so a layout can be recognised before it is chosen, which
// means showing the page rather than naming it. pdf.js rasterises page one to a
// canvas; the rest of the document is ignored — `bidding-sheets` in particular
// pages by auction length and can return several sheets for one preview.

import * as pdfjs from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl

/// -> a data URL of page 1, no wider or taller than `max`.
export async function firstPageImage(pdfBytes, max = 320) {
  // pdf.js takes ownership of the buffer it is given, and the caller may still
  // want the bytes for the download, so hand it a copy.
  const doc = await pdfjs.getDocument({ data: pdfBytes.slice(), disableAutoFetch: true }).promise
  try {
    const page = await doc.getPage(1)
    const base = page.getViewport({ scale: 1 })
    const scale = Math.min(max / base.width, max / base.height)
    const viewport = page.getViewport({ scale })

    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.ceil(viewport.width))
    canvas.height = Math.max(1, Math.ceil(viewport.height))
    const canvasContext = canvas.getContext('2d')
    // Pages are drawn on transparency; without a white ground the diagrams sit
    // on whatever the page behind them is, which in dark mode is unreadable.
    canvasContext.fillStyle = '#fff'
    canvasContext.fillRect(0, 0, canvas.width, canvas.height)

    await page.render({ canvasContext, viewport, canvas }).promise
    return { url: canvas.toDataURL('image/png'), width: canvas.width, height: canvas.height }
  } finally {
    // Frees the worker's copy; without it a gallery of six leaks six documents
    // every time the selection changes.
    doc.destroy()
  }
}
