import { describe, expect, it } from 'vitest'
import { catalog, initialState, recipes } from './data'
import { addIngredients, dateKey, mergeItem, mondayOfWeek } from './shopping'

describe('shopping behavior', () => {
  it('merges matching ingredients and preserves quantities and descriptions', () => {
    const pasta = catalog.find((p) => p.id === 'pasta')!
    const items = mergeItem([], pasta, 200, 'g', 'Whole wheat')
    const result = addIngredients(items, recipes[0].ingredients)
    expect(result.filter((p) => p.id === 'pasta')).toHaveLength(1)
    expect(result.find((p) => p.id === 'pasta')).toMatchObject({
      quantity: 600,
      unit: 'g',
      note: 'Whole wheat',
    })
  })
  it('does not add incompatible units together', () => {
    const product = catalog.find((p) => p.id === 'pasta')!
    const result = mergeItem(
      mergeItem([], product, 1, 'pack'),
      product,
      400,
      'g',
      '',
      true,
    )
    expect(result[0]).toMatchObject({
      quantity: 1,
      unit: 'pack',
      note: '',
      amounts: [
        { unit: 'g', quantity: 400 },
        { unit: 'pack', quantity: 1 },
      ],
    })
  })
  it('reactivates checked ingredients with the newly needed quantity', () => {
    const checked = initialState().items.filter((p) => p.id === 'cucumber')
    const result = mergeItem(
      checked,
      catalog.find((p) => p.id === 'cucumber')!,
      2,
      'pc',
    )
    expect(result[0]).toMatchObject({ checked: false, quantity: 2 })
  })
  it('computes current and next weeks across a year boundary', () => {
    const date = new Date(2026, 11, 31, 12)
    expect(dateKey(mondayOfWeek(date))).toBe('2026-12-28')
    expect(dateKey(mondayOfWeek(date, 1))).toBe('2027-01-04')
  })
})
