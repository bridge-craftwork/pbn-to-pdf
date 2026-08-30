import { afterEach, describe, expect, it, vi } from 'vitest'
import { fetchCatalogue, fetchPbn, lessonUrl } from './baker.js'

const ok = (body) => ({ ok: true, status: 200, json: async () => body, text: async () => body })

afterEach(() => vi.unstubAllGlobals())

describe('lessonUrl', () => {
  it('builds a raw.githubusercontent URL from a lesson id', () => {
    expect(lessonUrl('Blackwood')).toMatch(/Baker-Bridge\/main\/bridge-classroom\/Blackwood\.pbn$/)
  })

  // Ids come from a JSON file we do not own; one with a space or a slash must
  // not be able to escape the path.
  it('escapes ids', () => {
    expect(lessonUrl('a b/c')).toMatch(/a%20b%2Fc\.pbn$/)
  })
})

describe('fetchCatalogue', () => {
  it('flattens categories and defaults the optional fields', async () => {
    vi.stubGlobal('fetch', async () =>
      ok({
        categories: [
          { id: 'basic', name: 'Basic', lessons: [{ id: 'Major', name: 'Major Suit Openings' }] },
          { id: 'empty', name: 'Empty' },
        ],
      }),
    )
    const cats = await fetchCatalogue()
    expect(cats).toHaveLength(2)
    expect(cats[0].lessons[0]).toEqual({
      id: 'Major', name: 'Major Suit Openings', description: '', difficulty: '',
    })
    expect(cats[1].lessons).toEqual([])
  })

  it('reports an HTTP failure rather than throwing on undefined', async () => {
    vi.stubGlobal('fetch', async () => ({ ok: false, status: 404 }))
    await expect(fetchCatalogue()).rejects.toThrow(/404/)
  })
})

describe('fetchPbn', () => {
  it('returns the body', async () => {
    vi.stubGlobal('fetch', async () => ok('[Event "x"]'))
    expect(await fetchPbn('https://example.test/a.pbn')).toBe('[Event "x"]')
  })

  // A CORS refusal rejects with a bare "Failed to fetch"; the visitor needs to
  // be told what that actually means for a URL they chose.
  it('explains a network/CORS rejection', async () => {
    vi.stubGlobal('fetch', async () => {
      throw new TypeError('Failed to fetch')
    })
    await expect(fetchPbn('https://example.test/a.pbn')).rejects.toThrow(/cross-origin/)
  })

  it('propagates an abort untouched', async () => {
    vi.stubGlobal('fetch', async () => {
      throw Object.assign(new Error('aborted'), { name: 'AbortError' })
    })
    await expect(fetchPbn('https://example.test/a.pbn')).rejects.toMatchObject({ name: 'AbortError' })
  })
})
