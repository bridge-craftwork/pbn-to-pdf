// Bundle several rendered PDFs into one download.
//
// Store-only: a PDF's content streams are already deflated, so compressing them
// again costs time and saves almost nothing.

import { zipSync, strToU8 } from 'fflate'

/// `files` is [{ name, bytes }] -> a Blob. Duplicate names get a numeric
/// suffix; a zip with two identical entry names is not readable everywhere.
export function makeZip(files, readme) {
  const seen = new Map()
  const entries = {}
  for (const { name, bytes } of files) {
    let unique = name
    if (seen.has(name)) {
      const n = seen.get(name) + 1
      seen.set(name, n)
      const dot = name.lastIndexOf('.')
      unique = dot > 0 ? `${name.slice(0, dot)} (${n})${name.slice(dot)}` : `${name} (${n})`
    } else {
      seen.set(name, 1)
    }
    entries[unique] = bytes
  }
  if (readme) entries['README.txt'] = strToU8(readme)
  return new Blob([zipSync(entries, { level: 0 })], { type: 'application/zip' })
}
