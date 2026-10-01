import { describe, expect, it } from 'vitest'
import { catalog, categories, initialState } from './data'
import { changeGroup } from './groups'

describe('group edits', () => {
  it('renames built-in groups without losing assignments or aisle order', () => {
    const state = initialState()
    state.groups = [...categories]
    const product = catalog.find((product) => product.id === 'apples')!
    state.items = [
      { ...product, quantity: 2, unit: 'pc', note: '', checked: true },
    ]
    state.customProducts = [
      {
        id: 'local',
        name: 'Local apples',
        icon: '🍎',
        category: product.category,
      },
    ]
    const position = state.stores[0].order.indexOf(product.category)
    const result = changeGroup(state, product.category, 'Fresh fruit')
    expect(result.productGroups?.apples).toBe('Fresh fruit')
    expect(result.items[0]).toMatchObject({
      category: 'Fresh fruit',
      checked: true,
      quantity: 2,
    })
    expect(result.customProducts[0].category).toBe('Fresh fruit')
    expect(result.stores[0].order[position]).toBe('Fresh fruit')
    expect(state.items[0].category).toBe(product.category)
  })
  it('merges a deleted group into another while keeping one aisle per group', () => {
    const state = initialState()
    state.groups = [...categories]
    const source = catalog.find((product) => product.id === 'apples')!.category
    const target = state.groups!.find((group) => group !== source)!
    const result = changeGroup(state, source, target, true)
    expect(result.groups).not.toContain(source)
    expect(result.productGroups?.apples).toBe(target)
    expect(
      result.stores[0].order.filter((group) => group === target),
    ).toHaveLength(1)
    expect(result.stores[0].order).not.toContain(source)
    expect(() => changeGroup(state, source, source, true)).toThrow()
    expect(() => changeGroup(state, source, target)).toThrow()
  })
})
