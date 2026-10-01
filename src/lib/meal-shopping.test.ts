import { describe, expect, it } from 'vitest'
import { catalog, initialState, recipes } from './data'
import type { AppState, ShoppingItem } from './data'
import {
  editShoppingItem,
  getShoppingItems,
  preparePlanShopping,
  removeShoppingItem,
  shoppingQuantity,
  toggleShoppingItem,
  confirmPantry,
} from './meal-shopping'

const now = new Date(2026, 8, 30, 12)
const today = '2026-09-30'
const tomorrow = '2026-10-01'
function fixture(overrides: Partial<AppState> = {}): AppState {
  return {
    ...initialState(),
    items: [],
    plan: { [today]: 'green-pasta' },
    planCoverage: {},
    ...overrides,
  }
}
function pasta(quantity: number, unit = 'g'): ShoppingItem {
  return {
    ...catalog.find((product) => product.id === 'pasta')!,
    quantity,
    unit,
    checked: false,
    note: 'Whole wheat',
  }
}
function find(state: AppState, id = 'pasta') {
  return getShoppingItems(state, now).find((item) => item.id === id)
}

describe('automatic meal shopping', () => {
  it('removes restored groceries from pantry immediately, retaining unrelated stock and shopping details', () => {
    const pantry = {
      pasta: {
        amounts: [{ quantity: 400, unit: 'g' }],
        confirmed: false,
        observedAt: now.toISOString(),
        expiresAt: null,
        note: '',
      },
      rice: {
        amounts: [{ quantity: 300, unit: 'g' }],
        confirmed: true,
        observedAt: now.toISOString(),
        expiresAt: null,
        note: '',
      },
    }
    const checked = {
      ...fixture(),
      plan: {},
      items: [{ ...pasta(800), checked: true }],
      pantry,
    }
    const restored = toggleShoppingItem(checked, 'pasta', now)
    expect(restored.pantry?.pasta).toBeUndefined()
    expect(restored.pantry?.rice).toEqual(pantry.rice)
    expect(restored.items[0]).toMatchObject({
      quantity: 800,
      note: 'Whole wheat',
      checked: false,
    })
    expect(checked.pantry.pasta).toEqual(pantry.pasta)
    expect(JSON.parse(JSON.stringify(restored)).pantry.pasta).toBeUndefined()
  })
  it('allocates confirmed pantry totals to purchased reservations before using the remainder', () => {
    const purchased = toggleShoppingItem(fixture(), 'pasta', now)
    const planned = {
      ...purchased,
      plan: { ...purchased.plan, [tomorrow]: 'green-pasta' },
    }
    const enough = confirmPantry(
      planned,
      { productId: 'pasta', quantity: 0.6, unit: 'kg', expiresAt: null },
      now,
    )
    expect(enough.pantry?.pasta.amounts).toEqual([{ quantity: 200, unit: 'g' }])
    expect(find(enough)).toMatchObject({ quantity: 200, checked: false })
    const shortage = confirmPantry(
      planned,
      { productId: 'pasta', quantity: 100, unit: 'g', expiresAt: null },
      now,
    )
    expect(shortage.planCoverage?.[today].pasta).toEqual([
      { quantity: 100, unit: 'g' },
    ])
    expect(shortage.pantry?.pasta.amounts).toEqual([])
    expect(find(shortage)).toMatchObject({ quantity: 700, checked: false })
    const empty = confirmPantry(
      planned,
      { productId: 'pasta', quantity: 0, unit: 'g', expiresAt: null },
      now,
    )
    expect(find(empty)).toMatchObject({ quantity: 800, checked: false })
  })
  it('adds upcoming ingredients once, sums repeated meals, and ignores past/out-of-range days', () => {
    const state = fixture({
      plan: {
        '2026-09-29': 'green-pasta',
        [today]: 'green-pasta',
        [tomorrow]: 'green-pasta',
        '2026-10-11': 'green-pasta',
        '2026-10-12': 'green-pasta',
      },
    })
    expect(find(state)).toMatchObject({
      quantity: 1200,
      unit: 'g',
      autoAdded: true,
    })
    expect(getShoppingItems(state, now)).toEqual(getShoppingItems(state, now))
    expect(state.items).toEqual([])
    expect(
      getShoppingItems(state, now).filter((item) => item.id === 'pasta'),
    ).toHaveLength(1)
  })
  it('tops up existing quantities and keeps manually entered groceries after removing a meal', () => {
    const state = fixture({ items: [pasta(200)] })
    expect(find(state)).toMatchObject({
      quantity: 400,
      autoAdded: true,
      note: 'Whole wheat',
    })
    expect(find(fixture({ items: [pasta(500)] }))).toMatchObject({
      quantity: 500,
      autoAdded: false,
    })
    expect(find({ ...state, plan: {} })).toMatchObject({
      quantity: 200,
      autoAdded: false,
    })
    const swapped = { ...state, plan: { [today]: 'tomato-soup' } }
    expect(find(swapped)).toMatchObject({ quantity: 200 })
    expect(find(swapped, 'spinach')).toBeUndefined()
    expect(find(swapped, 'chopped-tomatoes')).toMatchObject({ quantity: 800 })
  })
  it('converts compatible units while retaining distinct amounts for unlike units on one tile', () => {
    expect(find(fixture({ items: [pasta(1, 'kg')] }))).toMatchObject({
      quantity: 1,
      unit: 'kg',
      autoAdded: false,
    })
    expect(find(fixture({ items: [pasta(0.2, 'kg')] }))).toMatchObject({
      quantity: 0.4,
      unit: 'kg',
      autoAdded: true,
    })
    const item = find(fixture({ items: [pasta(1, 'pack')] }))!
    expect(shoppingQuantity(item)).toBe('400 g + 1 pack')
    expect(item.amounts).toHaveLength(2)
  })
  it('keeps checked ingredients purchased through reloads and lets a tap restore them', () => {
    const state = toggleShoppingItem(fixture(), 'pasta', now)
    expect(find(state)).toMatchObject({ checked: true })
    const reloaded = JSON.parse(JSON.stringify(state)) as AppState
    expect(find({ ...reloaded, storeId: 'lidl' })).toMatchObject({
      checked: true,
    })
    expect(find(toggleShoppingItem(reloaded, 'pasta', now))).toMatchObject({
      checked: false,
      quantity: 400,
    })
  })
  it('adds only new demand after purchase and can undo the latest check without losing earlier purchases', () => {
    const purchased = toggleShoppingItem(fixture(), 'pasta', now)
    const nextMeal = {
      ...purchased,
      plan: { ...purchased.plan, [tomorrow]: 'green-pasta' },
    }
    expect(find(nextMeal)).toMatchObject({ quantity: 400, checked: false })
    const boughtAgain = toggleShoppingItem(nextMeal, 'pasta', now)
    expect(find(boughtAgain)).toMatchObject({ checked: true })
    expect(find(toggleShoppingItem(boughtAgain, 'pasta', now))).toMatchObject({
      quantity: 400,
      checked: false,
    })
  })
  it('preserves a manual baseline when checking and restoring a plan top-up', () => {
    const state = fixture({ items: [pasta(200)] })
    const restored = toggleShoppingItem(
      toggleShoppingItem(state, 'pasta', now),
      'pasta',
      now,
    )
    expect(find(restored)).toMatchObject({ quantity: 400, note: 'Whole wheat' })
    expect(find({ ...restored, plan: {} })).toMatchObject({
      quantity: 200,
      checked: false,
    })
  })
  it('removes and skips current requirements without suppressing ingredients for a newly added meal', () => {
    const removed = removeShoppingItem(fixture(), 'pasta', now)
    expect(find(removed)).toBeUndefined()
    expect(
      find({
        ...removed,
        plan: { ...removed.plan, [tomorrow]: 'green-pasta' },
      }),
    ).toMatchObject({ quantity: 400 })
  })
  it('lets automatic notes follow a meal while an explicit quantity edit creates a manual grocery', () => {
    const state = fixture()
    const item = find(state)!
    const edits = {
      category: item.category,
      quantity: item.quantity,
      unit: item.unit,
      note: 'Kids prefer penne',
    }
    const withNote = editShoppingItem(state, item, edits, now)
    expect(find(withNote)).toMatchObject({
      note: 'Kids prefer penne',
      autoAdded: true,
    })
    expect(find({ ...withNote, plan: {} })).toBeUndefined()
    const withQuantity = editShoppingItem(
      state,
      item,
      { ...edits, quantity: 600 },
      now,
    )
    expect(find({ ...withQuantity, plan: {} })).toMatchObject({
      quantity: 600,
      autoAdded: false,
    })
    const less = editShoppingItem(state, item, { ...edits, quantity: 100 }, now)
    expect(find(less)).toMatchObject({ quantity: 100, autoAdded: false })
    expect(
      find({ ...less, plan: { ...less.plan, [tomorrow]: 'green-pasta' } }),
    ).toMatchObject({ quantity: 400, autoAdded: true })
    const purchased = toggleShoppingItem(state, 'pasta', now)
    const renewed = {
      ...purchased,
      plan: { ...purchased.plan, [tomorrow]: 'green-pasta' },
    }
    const overridden = editShoppingItem(
      renewed,
      find(renewed)!,
      { ...edits, quantity: 100 },
      now,
    )
    expect(find(overridden)).toMatchObject({
      quantity: 100,
      autoAdded: false,
      checked: false,
    })
  })
  it('credits checked items from the old prototype by their actual quantity', () => {
    const state = fixture({
      items: [{ ...pasta(200), checked: true }],
      planCoverage: undefined,
    })
    const migrated = preparePlanShopping(state, now)
    expect(find(migrated)).toMatchObject({ quantity: 200, checked: false })
    const boughtRemaining = toggleShoppingItem(migrated, 'pasta', now)
    expect(find(boughtRemaining)).toMatchObject({ checked: true })
    expect(
      find(toggleShoppingItem(boughtRemaining, 'pasta', now)),
    ).toMatchObject({ quantity: 200, checked: false })
    const fullyCovered = preparePlanShopping(
      fixture({
        items: [{ ...pasta(400), checked: true }],
        planCoverage: undefined,
      }),
      now,
    )
    expect(find(toggleShoppingItem(fullyCovered, 'pasta', now))).toMatchObject({
      quantity: 400,
      checked: false,
    })
  })
  it('handles duplicate ingredient rows and a later increase in a custom recipe without spending coverage twice', () => {
    const customRecipe = {
      ...recipes[0],
      id: 'custom',
      ingredients: [
        { productId: 'pasta', quantity: 0.2, unit: 'kg' },
        { productId: 'pasta', quantity: 200, unit: 'g' },
      ],
    }
    const state = fixture({
      customRecipes: [customRecipe],
      plan: { [today]: 'custom' },
    })
    expect(find(state)).toMatchObject({ quantity: 400 })
    const purchased = toggleShoppingItem(state, 'pasta', now)
    expect(find(purchased)).toMatchObject({ checked: true })
    expect(
      find({
        ...purchased,
        customRecipes: [
          {
            ...customRecipe,
            ingredients: [{ productId: 'pasta', quantity: 600, unit: 'g' }],
          },
        ],
      }),
    ).toMatchObject({ quantity: 200, checked: false })
  })
})
