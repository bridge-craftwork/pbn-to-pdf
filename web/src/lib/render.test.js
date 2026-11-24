import { describe, expect, it } from 'vitest'
import { drawsPlay, layoutLabel, pagesByBoard, usesCardArt } from './render.js'

describe('layoutLabel', () => {
  it('names every layout the engine reports', () => {
    for (const id of [
      'analysis', 'bidding-sheets', 'declarers-plan-1up',
      'declarers-plan-2up', 'declarers-plan', 'dealer-summary', 'hand-record',
    ]) {
      expect(layoutLabel(id)).not.toBe(id)
    }
  })

  // A layout added to the engine must still render a usable menu entry rather
  // than an empty one.
  it('falls back to the id it does not know', () => {
    expect(layoutLabel('brand-new')).toBe('brand-new')
  })
})

describe('usesCardArt', () => {
  it("is true only for the declarer's plan family", () => {
    expect(['declarers-plan', 'declarers-plan-1up', 'declarers-plan-2up'].every(usesCardArt)).toBe(true)
    expect(['analysis', 'bidding-sheets', 'dealer-summary'].some(usesCardArt)).toBe(false)
  })
})

describe('drawsPlay', () => {
  it('is true only for the analysis layout', () => {
    expect(drawsPlay('analysis')).toBe(true)
    expect(['hand-record', 'declarers-plan', 'bidding-sheets'].some(drawsPlay)).toBe(false)
  })
})

describe('pagesByBoard', () => {
  it('gives each board the first page it was drawn on', () => {
    const map = {
      blocks: [
        { kind: 'board', board: 1, page: 1 },
        { kind: 'board', board: 2, page: 1 },
        { kind: 'board', board: 2, page: 2 },
        { kind: 'board', board: 4, page: 3 },
      ],
    }
    expect(pagesByBoard(map)).toEqual([1, 1, 0, 3])
  })

  it('is empty without blocks', () => {
    expect(pagesByBoard({})).toEqual([])
  })
})
