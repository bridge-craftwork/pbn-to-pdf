import { afterEach, describe, expect, it, vi } from 'vitest'
import { assetUrl, fetchLibrary, fetchSetViews, VIEW_FOR_LAYOUT } from './baker.js'
import { LAYOUT_ORDER } from './render.js'

const ok = (body) => ({ ok: true, status: 200, json: async () => body, text: async () => body })
afterEach(() => vi.unstubAllGlobals())

describe('assetUrl', () => {
  it('encodes spaces', () => {
    expect(assetUrl('a b/c.pbn')).toMatch(/a%20b\/c\.pbn$/)
  })

  // The producer emits "(20 hands)  NESW.pbn" with two spaces. Losing one
  // yields a 404 that looks like a missing file rather than a bad URL.
  it('preserves a double space', () => {
    expect(assetUrl('x (20 hands)  NESW.pbn')).toMatch(/%20%20NESW/)
  })

  // encodeURI leaves # alone because it is legal in a URI; in a path it would
  // truncate everything after it into a fragment.
  it('escapes a hash in a filename', () => {
    expect(assetUrl('Set #1.pbn')).toMatch(/Set%20%231\.pbn$/)
    expect(assetUrl('Set #1.pbn')).not.toMatch(/#/)
  })
})

describe('VIEW_FOR_LAYOUT', () => {
  it('maps every layout the gallery shows', () => {
    for (const layout of LAYOUT_ORDER) expect(VIEW_FOR_LAYOUT[layout]).toBeTruthy()
  })

  // The mapping is the point of using Rotations at all: South always declares,
  // North-South alternates, Full Table shows four hands.
  it('sends the declarer plans to South and bidding sheets to North-South', () => {
    expect(VIEW_FOR_LAYOUT['declarers-plan']).toBe('South')
    expect(VIEW_FOR_LAYOUT['declarers-plan-1up']).toBe('South')
    expect(VIEW_FOR_LAYOUT['declarers-plan-2up']).toBe('South')
    expect(VIEW_FOR_LAYOUT['bidding-sheets']).toBe('North-South')
    expect(VIEW_FOR_LAYOUT['dealer-summary']).toBe('Full Table')
  })
})

const MANIFEST = {
  generatedAt: '2026-07-14T19:59:38Z',
  categories: [
    {
      name: '4. Declarer Play',
      lessons: [
        {
          name: 'Suit Establishment',
          path: '4. Declarer Play/Suit Establishment',
          boards: 20,
          all: { boards: 20, views: { South: { pbn: 'a-south.pbn' } } },
          setSizes: [
            { size: 4, sets: [{ set: 1, boards: 4, views: { South: { pbn: 's1.pbn' } } }] },
            { size: 6, sets: [{ set: 1, boards: 6, views: { South: { pbn: 's2.pbn' } } }] },
          ],
        },
      ],
    },
    { name: '9. Empty' },
  ],
}

describe('fetchLibrary', () => {
  it('flattens categories into lesson rows with their sets', async () => {
    vi.stubGlobal('fetch', async () => ok(MANIFEST))
    const { lessons } = await fetchLibrary()
    expect(lessons).toHaveLength(1)
    const [l] = lessons
    expect(l.name).toBe('Suit Establishment')
    expect(l.boards).toBe(20)
    // The whole lesson first, then one entry per set.
    expect(l.sets.map((s) => s.id)).toEqual(['all', '4-1', '6-1'])
  })

  // Categories are numbered so they sort; the number is noise in the table.
  it('strips the ordering prefix from the category label', async () => {
    vi.stubGlobal('fetch', async () => ok(MANIFEST))
    const { lessons } = await fetchLibrary()
    expect(lessons[0].category).toBe('4. Declarer Play')
    expect(lessons[0].categoryLabel).toBe('Declarer Play')
  })

  it('tolerates a category with no lessons', async () => {
    vi.stubGlobal('fetch', async () => ok(MANIFEST))
    await expect(fetchLibrary()).resolves.toBeTruthy()
  })

  it('reports an HTTP failure rather than throwing on undefined', async () => {
    vi.stubGlobal('fetch', async () => ({ ok: false, status: 404 }))
    await expect(fetchLibrary()).rejects.toThrow(/404/)
  })
})

describe('fetchSetViews', () => {
  const set = {
    views: {
      South: { pbn: 's.pbn' },
      'North-South': { pbn: 'ns.pbn' },
      'Full Table': { pbn: 'ft.pbn' },
    },
  }

  it('returns every rotation the gallery needs', async () => {
    vi.stubGlobal('fetch', async (u) => ok(`PBN:${u}`))
    const views = await fetchSetViews(set)
    expect(Object.keys(views).sort()).toEqual(['Full Table', 'North-South', 'South'])
    expect(views.South).toContain('s.pbn')
  })

  // One unreachable rotation must not cost the layouts that do not use it.
  it('keeps the rotations that resolve when one fails', async () => {
    vi.stubGlobal('fetch', async (u) =>
      u.includes('ft.pbn') ? { ok: false, status: 404 } : ok('data'),
    )
    const views = await fetchSetViews(set)
    expect(views['Full Table']).toBeNull()
    expect(views.South).toBe('data')
  })

  it('fails when nothing resolves', async () => {
    vi.stubGlobal('fetch', async () => ({ ok: false, status: 404 }))
    await expect(fetchSetViews(set)).rejects.toThrow(/could be fetched/)
  })
})
