// pdf.js, with its worker pointed at the bundled copy. Import it from here so
// the worker is configured once, whichever module loads it first.

import * as pdfjs from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl

export { pdfjs }
