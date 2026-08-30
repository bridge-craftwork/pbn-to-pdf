import { describe, expect, it } from 'vitest'
import { layoutLabel, usesCardArt } from './render.js'

describe('layoutLabel', () => {
  it('names every layout the engine reports', () => {
    for (const id of [
      'analysis', 'bidding-sheets', 'declarers-plan-1up',
      'declarers-plan-2up', 'declarers-plan', 'dealer-summary',
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
