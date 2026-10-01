import { describe, expect, it } from 'vitest'
import { initialState, catalog } from './data'
import {
  archiveProduct,
  deleteProduct,
  deleteRecipe,
  getProducts,
  saveProduct,
} from './catalog'
import { diffState, optimistic } from './sync'
import { changeGroup } from './groups'

describe('settings catalog management', () => {
  it('keeps product identity and shopping details while overriding a bundled product', () => {
    const state = initialState()
    const apple = catalog.find((product) => product.id === 'apples')!
    state.items = [
      { ...apple, quantity: 4, unit: 'pc', note: 'Pink lady', checked: true },
    ]
    const changed = saveProduct(state, {
      ...apple,
      name: 'Eating apples',
      category: state.stores[0].order[1],
      aliases: ['sweet apples'],
      image: '/api/images/test-image',
    })
    expect(
      getProducts(changed).find((product) => product.id === 'apples'),
    ).toMatchObject({ name: 'Eating apples', aliases: ['sweet apples'] })
    expect(changed.items[0]).toMatchObject({
      name: 'Eating apples',
      checked: true,
      quantity: 4,
      note: 'Pink lady',
    })
    expect(
      diffState(state, changed).some(
        (change) => change.collection === 'productDetails',
      ),
    ).toBe(true)
    expect(
      optimistic(changed, {
        type: 'addItem',
        productId: 'apples',
        quantity: 1,
        unit: 'pc',
        note: '',
        accumulate: false,
      }).items[0].name,
    ).toBe('Eating apples')
    const migrated = changeGroup(
      changed,
      changed.items[0].category,
      'Fresh produce',
    )
    expect(migrated.productDetails?.apples.category).toBe('Fresh produce')
  })
  it('archives suggestions without breaking recipe identity and restores the customization', () => {
    const state = initialState()
    const apple = catalog.find((product) => product.id === 'apples')!
    const changed = archiveProduct(
      saveProduct(state, { ...apple, name: 'Eating apples' }),
      apple.id,
      true,
    )
    expect(
      getProducts(changed, false).some((product) => product.id === apple.id),
    ).toBe(false)
    expect(
      getProducts(changed).find((product) => product.id === apple.id)?.name,
    ).toBe('Eating apples')
    expect(
      getProducts(archiveProduct(changed, apple.id, false), false).find(
        (product) => product.id === apple.id,
      )?.name,
    ).toBe('Eating apples')
    expect(() => deleteProduct(changed, apple.id)).toThrow('Built-in')
  })
  it('rejects deleting referenced custom ingredients and cleans all live references after deletion', () => {
    const state = initialState()
    const custom = {
      id: 'family-item',
      name: 'Family item',
      category: state.stores[0].order[0],
      icon: '🛒',
      aliases: [],
    }
    const changed = saveProduct(state, custom)
    changed.customRecipes = [
      {
        id: 'family-recipe',
        name: 'Family recipe',
        subtitle: '',
        image: '/images/pasta.jpg',
        minutes: 10,
        servings: 4,
        tag: 'Family',
        steps: ['Cook'],
        ingredients: [{ productId: custom.id, quantity: 1, unit: 'pc' }],
      },
    ]
    changed.items = [
      { ...custom, quantity: 1, unit: 'pc', note: '', checked: false },
    ]
    changed.pantry = {
      [custom.id]: {
        amounts: [{ quantity: 1, unit: 'pc' }],
        confirmed: true,
        observedAt: new Date().toISOString(),
        expiresAt: null,
        note: '',
      },
    }
    expect(() => deleteProduct(changed, custom.id)).toThrow('used in a recipe')
    const removed = deleteProduct(
      deleteRecipe(changed, 'family-recipe'),
      custom.id,
    )
    expect(removed.customProducts).toHaveLength(0)
    expect(removed.items).toHaveLength(0)
    expect(removed.pantry).toEqual({})
    expect(removed.productGroups?.[custom.id]).toBeUndefined()
  })
})
