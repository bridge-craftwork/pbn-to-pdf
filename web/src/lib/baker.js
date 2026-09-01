// The Baker Bridge lesson library, read from the Rotations manifest.
//
// bridge-craftwork/Baker-Bridge is public and Unlicense, and raw.githubusercontent
// sends `access-control-allow-origin: *`, so the browser reads the manifest and
// the lessons directly — no proxy, no key.
//
// Rotations rather than bridge-classroom: the app export carries interactive
// control directives ([NEXT], [RESET], [ROTATE]) that would print onto a paper
// handout as "Make a Plan, then click NEXT."  Rotations is built with those
// stripped, and it adds the thing that matters here — each set is rotated for a
// seating, and the seating a layout wants is not the same one another wants.

const BASE =
  'https://raw.githubusercontent.com/bridge-craftwork/Baker-Bridge/main/Rotations/'

/// Which rotation each layout should be rendered from.
///
/// Not cosmetic. In the South view the student always declares, which is what a
/// declarer's plan is for; North-South alternates declarer between the partners,
/// which is what bidding practice wants; Full Table is the four-hand reference
/// the dealer and the summary sheets are built from.
export const VIEW_FOR_LAYOUT = {
  'declarers-plan': 'South',
  'declarers-plan-1up': 'South',
  'declarers-plan-2up': 'South',
  'bidding-sheets': 'North-South',
  'dealer-summary': 'Full Table',
  analysis: 'Full Table',
}

/// Opens on a declarer-play lesson, not a bidding one. Its boards carry an
/// opening lead, without which a declarer's plan has nothing to plan from, and
/// it shows the winner markers off well.
export const DEFAULT_LESSON = 'Suit Establishment'

export const assetUrl = (relative) => BASE + encodeURI(relative).replace(/#/g, '%23')

/// -> { generatedAt, contentHash, lessons: [{ id, category, name, boards, sets }] }
///
/// If this ever caches across sessions, key on `contentHash`, not `generatedAt`.
/// The hash is a digest of everything the manifest describes: it moves iff the
/// tree moves, and is identical across rebuilds of an unchanged tree.
/// `generatedAt` is pinned to the deal set so that rebuilding unchanged content
/// yields an unchanged file — which means a packaging-only change leaves it
/// standing still, and it cannot answer "is my copy current?".
///
/// Nothing caches today, deliberately. Measured against the live site, the
/// manifest costs 251 ms to fetch and 0.7 ms to parse, and raw.githubusercontent
/// already serves it with `max-age=300` and a strong ETag — so a second visit
/// revalidates with a bodyless 304. A localStorage layer would duplicate the
/// transport for a saving invisible beside the ~9 MB engine download.
///
/// Flattened deliberately: the manifest nests category > lesson > set size > set,
/// but a filterable table wants one row per lesson and a set chosen after.
export async function fetchLibrary(signal) {
  const res = await fetch(assetUrl('manifest.json'), { signal })
  if (!res.ok) throw new Error(`Could not load the lesson library (HTTP ${res.status})`)
  const manifest = await res.json()

  const lessons = []
  for (const category of manifest.categories ?? []) {
    for (const lesson of category.lessons ?? []) {
      const sets = []
      // The whole lesson in one file, when the manifest offers it.
      if (lesson.all?.views) {
        sets.push({
          id: 'all',
          label: `All ${lesson.all.boards ?? lesson.boards} boards`,
          // `short` is what a chip shows once its group is already labelled.
          short: 'All',
          boards: lesson.all.boards ?? lesson.boards,
          views: lesson.all.views,
        })
      }
      for (const size of lesson.setSizes ?? []) {
        for (const set of size.sets ?? []) {
          sets.push({
            id: `${size.size}-${set.set}`,
            label: `${size.size}-board · set ${set.set}`,
            group: `${size.size}-board sets`,
            short: String(set.set),
            boards: set.boards,
            views: set.views,
          })
        }
      }
      lessons.push({
        id: lesson.path || `${category.name}/${lesson.name}`,
        category: category.name,
        // Categories are numbered for ordering ("4. Declarer Play"); the number
        // is noise in a table that is already grouped.
        categoryLabel: (category.name || '').replace(/^\d+\.\s*/, ''),
        name: lesson.name,
        boards: lesson.boards,
        sets,
      })
    }
  }
  return { generatedAt: manifest.generatedAt, contentHash: manifest.contentHash, lessons }
}

/// Fetch the PBN for one set in one rotation.
export async function fetchSetView(set, view, signal) {
  const relative = set?.views?.[view]?.pbn
  if (!relative) throw new Error(`This set has no ${view} rotation.`)
  const res = await fetch(assetUrl(relative), { signal })
  if (!res.ok) throw new Error(`${view}: HTTP ${res.status}`)
  return res.text()
}

/// Every rotation a set offers, keyed by view, so each layout can be rendered
/// from the right one. Fetched together because the layout gallery shows all
/// six at once and they span all three rotations.
export async function fetchSetViews(set, signal) {
  const wanted = [...new Set(Object.values(VIEW_FOR_LAYOUT))]
  const results = await Promise.all(
    wanted.map(async (view) => {
      try {
        return [view, await fetchSetView(set, view, signal)]
      } catch (e) {
        if (e.name === 'AbortError') throw e
        return [view, null]
      }
    }),
  )
  const views = Object.fromEntries(results)
  if (Object.values(views).every((v) => v === null)) {
    throw new Error('None of this set’s files could be fetched.')
  }
  return views
}

/// Shared by the URL box and the file picker, so both report failures alike.
export async function fetchPbn(url, signal) {
  let res
  try {
    res = await fetch(url, { signal })
  } catch (e) {
    if (e.name === 'AbortError') throw e
    // fetch rejects without detail on a CORS refusal, the likeliest cause for an
    // arbitrary URL; "Failed to fetch" would leave the visitor guessing.
    throw new Error('Could not fetch that URL. The server may not allow cross-origin requests.')
  }
  if (!res.ok) throw new Error(`That URL returned HTTP ${res.status}.`)
  return res.text()
}
