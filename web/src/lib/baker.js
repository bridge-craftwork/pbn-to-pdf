// The Baker Bridge lesson library, fetched straight from GitHub.
//
// bridge-craftwork/Baker-Bridge is public and Unlicense, and raw.githubusercontent
// sends `access-control-allow-origin: *`, so the browser can read both the
// catalogue and the lessons with no proxy and no API key.
//
// `bridge-classroom/toc.json` lists 50 lessons across 7 categories; each
// lesson's `id` is also its filename, `bridge-classroom/<id>.pbn`.

const BASE =
  'https://raw.githubusercontent.com/bridge-craftwork/Baker-Bridge/main/bridge-classroom'

/// The lesson to show on first load, so the page does something before the
/// visitor has found a file. A declarer's-plan lesson, because that is the
/// layout the card art exists for.
export const DEFAULT_LESSON = { id: 'Blackwood', name: 'Blackwood' }

export const lessonUrl = (id) => `${BASE}/${encodeURIComponent(id)}.pbn`

/// -> [{ id, name, lessons: [{ id, name, description, difficulty }] }]
export async function fetchCatalogue(signal) {
  const res = await fetch(`${BASE}/toc.json`, { signal })
  if (!res.ok) throw new Error(`Could not load the lesson list (HTTP ${res.status})`)
  const toc = await res.json()
  return (toc.categories ?? []).map((c) => ({
    id: c.id,
    name: c.name,
    lessons: (c.lessons ?? []).map((l) => ({
      id: l.id,
      name: l.name || l.id,
      description: l.description || '',
      difficulty: l.difficulty || '',
    })),
  }))
}

export async function fetchLesson(id, signal) {
  return fetchPbn(lessonUrl(id), signal)
}

/// Shared by the library and the URL box, so both report failures the same way.
export async function fetchPbn(url, signal) {
  let res
  try {
    res = await fetch(url, { signal })
  } catch (e) {
    if (e.name === 'AbortError') throw e
    // fetch rejects without detail on a CORS refusal, which is the likeliest
    // cause for an arbitrary URL, so name it rather than surfacing "Failed to
    // fetch" and leaving the visitor to guess.
    throw new Error(
      'Could not fetch that URL. The server may not allow cross-origin requests.',
    )
  }
  if (!res.ok) throw new Error(`That URL returned HTTP ${res.status}.`)
  return res.text()
}
