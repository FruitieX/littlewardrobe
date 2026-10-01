import type { AppState, ShoppingItem } from './data'
import type { Change } from './api/Change'
import type { Operation } from './api/Operation'
import { resetApprovals } from './wardrobe'
import {
  editShoppingItem,
  getShoppingItems,
  removeShoppingItem,
  toggleShoppingItem,
  confirmPantry,
} from './meal-shopping'
import { addIngredients, mergeItem } from './shopping'
import { getProducts } from './catalog'

const lists = [
  'garments',
  'children',
  'locations',
  'saleListings',
  'items',
  'stores',
  'customProducts',
  'customRecipes',
  'recipeBook',
] as const
const maps = [
  'plan',
  'planCoverage',
  'pantry',
  'servings',
  'mealFeedback',
  'suggestionFeedback',
  'productImages',
  'productGroups',
  'productDetails',
] as const
const scalars = [
  'groups',
  'favoriteRecipeIds',
  'preferences',
  'archivedProductIds',
] as const
function equal(a: unknown, b: unknown) {
  return JSON.stringify(a ?? null) === JSON.stringify(b ?? null)
}
function value(value: unknown): Change['after'] {
  return JSON.parse(JSON.stringify(value ?? null))
}

export function diffState(before: AppState, after: AppState): Change[] {
  const changes: Change[] = []
  for (const collection of lists) {
    const previous = (before[collection] ?? []) as unknown as Record<
      string,
      unknown
    >[]
    const next = (after[collection] ?? []) as unknown as Record<
      string,
      unknown
    >[]
    for (const id of new Set(
      [...previous, ...next].map((entry) => String(entry.id)),
    )) {
      const a = previous.find((entry) => entry.id === id),
        b = next.find((entry) => entry.id === id)
      if (equal(a, b)) continue
      if (!a || !b)
        changes.push({
          collection,
          key: id,
          field: null,
          before: value(a),
          after: value(b),
        })
      else
        for (const field of new Set([...Object.keys(a), ...Object.keys(b)])) {
          if (field === 'autoAdded' || field === 'mealNames') continue
          if (!equal(a[field], b[field]))
            changes.push({
              collection,
              key: id,
              field,
              before: value(a[field]),
              after: value(b[field]),
            })
        }
    }
  }
  for (const collection of maps) {
    const previous = (before[collection] ?? {}) as Record<string, unknown>,
      next = (after[collection] ?? {}) as Record<string, unknown>
    for (const key of new Set([...Object.keys(previous), ...Object.keys(next)]))
      if (!equal(previous[key], next[key]))
        changes.push({
          collection,
          key,
          field: null,
          before: value(previous[key]),
          after: value(next[key]),
        })
  }
  for (const collection of scalars) {
    if (
      collection === 'preferences' &&
      before.preferences &&
      after.preferences
    ) {
      for (const field of Object.keys(after.preferences) as (keyof NonNullable<
        AppState['preferences']
      >)[])
        if (!equal(before.preferences[field], after.preferences[field]))
          changes.push({
            collection,
            key: '',
            field,
            before: value(before.preferences[field]),
            after: value(after.preferences[field]),
          })
    } else if (!equal(before[collection], after[collection]))
      changes.push({
        collection,
        key: '',
        field: null,
        before: value(before[collection]),
        after: value(after[collection]),
      })
  }
  return changes
}

export function cleanItem(
  item: ShoppingItem,
): Extract<Operation, { type: 'editItem' }>['expected'] {
  const { autoAdded: _auto, mealNames: _meals, ...stored } = item
  return { ...stored, aliases: item.aliases ?? [] }
}

export function optimistic(state: AppState, operation: Operation): AppState {
  switch (operation.type) {
    case 'checkItem': {
      // A queue entry records a target state, so replay is never a second toggle.
      const current = getShoppingItems(state).find(
        (item) => item.id === operation.id,
      )
      if (current?.checked === operation.checked) return state
      return toggleShoppingItem(state, operation.id)
    }
    case 'addItem': {
      const product = getProducts(state).find(
        (product) => product.id === operation.productId,
      )
      return product
        ? {
            ...state,
            items: mergeItem(
              state.items,
              product,
              operation.quantity,
              operation.unit,
              operation.note,
              operation.accumulate,
            ),
          }
        : state
    }
    case 'addIngredients':
      return {
        ...state,
        items: addIngredients(
          state.items,
          operation.ingredients,
          operation.multiplier,
          state.customProducts,
          getProducts(state),
        ),
      }
    case 'removeItem':
      return removeShoppingItem(state, operation.id)
    case 'editItem':
      return editShoppingItem(
        state,
        operation.expected as ShoppingItem,
        operation,
      )
    case 'observePantry':
      return confirmPantry(state, operation)
    case 'mealFeedback':
      return {
        ...state,
        mealFeedback: {
          ...state.mealFeedback,
          [operation.day]: {
            recipeId: operation.recipeId,
            status: operation.status,
            servings:
              state.servings?.[operation.day] ??
              state.preferences?.servings ??
              4,
            at: new Date().toISOString(),
            rating: operation.rating,
            consumed: operation.consume,
          },
        },
      }
    case 'finishTrip':
      return { ...state, items: state.items.filter((item) => !item.checked) }
    case 'patch': {
      const next = structuredClone(state) as unknown as Record<string, unknown>
      for (const change of operation.changes) {
        if ((lists as readonly string[]).includes(change.collection)) {
          const list = next[change.collection] as Record<string, unknown>[]
          const index = list.findIndex((entry) => entry.id === change.key)
          if (change.field) {
            if (index >= 0) list[index][change.field] = change.after
          } else if (change.after === null) {
            if (index >= 0) list.splice(index, 1)
          } else if (index >= 0)
            list[index] = change.after as Record<string, unknown>
          else list.push(change.after as Record<string, unknown>)
        } else if ((maps as readonly string[]).includes(change.collection)) {
          const map = (next[change.collection] ??= {}) as Record<
            string,
            unknown
          >
          if (change.after === null) delete map[change.key]
          else map[change.key] = change.after
        } else if (change.field)
          (next[change.collection] as Record<string, unknown>)[change.field] =
            change.after
        else next[change.collection] = change.after
      }
      const result = next as unknown as AppState
      for (const g of result.garments ?? [])
        result.saleListings = resetApprovals(
          result.saleListings ?? [],
          state.garments?.find((old) => old.id === g.id),
          g,
        )
      return result
    }
    default:
      return state
  }
}
