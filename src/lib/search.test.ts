import { describe, expect, it } from 'vitest'
import { matchesSearch } from './search'

describe('clothing search', () => {
  it('matches words across clothing fields regardless of order or accents', () => {
    const fields = ['Crème shirt', 'H&M', '98', 'cotton', 'small sleeve mark']
    expect(matchesSearch(fields, '98 creme')).toBe(true)
    expect(matchesSearch(fields, 'COTTON mark')).toBe(true)
    expect(matchesSearch(fields, 'cotton stain')).toBe(false)
    expect(matchesSearch(fields, '   ')).toBe(true)
  })
})
