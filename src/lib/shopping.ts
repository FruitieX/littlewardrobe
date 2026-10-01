import type {
  AppState,
  Ingredient,
  Product,
  Recipe,
  ShoppingItem,
} from './data'
import { catalog } from './data'

export function mergeItem(
  items: ShoppingItem[],
  product: Product,
  quantity = 1,
  unit = 'pc',
  note = '',
  accumulate = false,
): ShoppingItem[] {
  const existing = items.find((item) => item.id === product.id)
  if (!existing)
    return [...items, { ...product, quantity, unit, note, checked: false }]
  const normalize = (quantity: number, unit: string) => {
    const lower = unit.trim().toLowerCase()
    const normalized = (
      {
        kg: ['g', 1000],
        l: ['ml', 1000],
        litre: ['ml', 1000],
        litres: ['ml', 1000],
        liter: ['ml', 1000],
        liters: ['ml', 1000],
        gram: ['g', 1],
        grams: ['g', 1],
        pcs: ['pc', 1],
        piece: ['pc', 1],
        pieces: ['pc', 1],
        clove: ['cloves', 1],
      } as Record<string, [string, number]>
    )[lower] ?? [lower, 1]
    return { unit: normalized[0], quantity: quantity * normalized[1] }
  }
  return items.map((item) => {
    if (item.id !== product.id) return item
    const added = normalize(quantity, unit),
      previous = normalize(item.quantity, item.unit)
    let extras = item.amounts
    if (accumulate && !item.checked && added.unit !== previous.unit) {
      const values = [
        ...(extras ?? [{ quantity: item.quantity, unit: item.unit }]),
        added,
      ].map((a) => normalize(a.quantity, a.unit))
      extras = Object.entries(
        values.reduce(
          (sum, a) => ({ ...sum, [a.unit]: (sum[a.unit] ?? 0) + a.quantity }),
          {} as Record<string, number>,
        ),
      )
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([unit, quantity]) => ({ unit, quantity }))
    }
    return {
      ...item,
      origin: undefined,
      checked: false,
      quantity: item.checked
        ? quantity
        : accumulate && added.unit === previous.unit
          ? item.quantity + added.quantity / normalize(1, item.unit).quantity
          : item.quantity,
      unit: item.checked ? unit : item.unit,
      note: [...new Set([item.note, note.trim()].filter(Boolean))].join(' · '),
      amounts: item.checked ? undefined : extras,
      planCredits: item.checked ? undefined : item.planCredits,
      manualAmount: item.checked ? undefined : item.manualAmount,
    }
  })
}

export function addIngredients(
  items: ShoppingItem[],
  ingredients: Ingredient[],
  multiplier = 1,
  customProducts: Product[] = [],
  products: Product[] = [...catalog, ...customProducts],
): ShoppingItem[] {
  return ingredients.reduce((result, ingredient) => {
    const product = products.find((p) => p.id === ingredient.productId)
    return product
      ? mergeItem(
          result,
          product,
          ingredient.quantity * multiplier,
          ingredient.unit,
          ingredient.note,
          true,
        )
      : result
  }, items)
}

export function addRecipes(
  items: ShoppingItem[],
  recipes: Recipe[],
): ShoppingItem[] {
  return recipes.reduce(
    (result, recipe) => addIngredients(result, recipe.ingredients),
    items,
  )
}

export function mondayOfWeek(date = new Date(), offset = 0): Date {
  const result = new Date(date)
  result.setHours(12, 0, 0, 0)
  result.setDate(result.getDate() - ((result.getDay() + 6) % 7) + offset * 7)
  return result
}
export function dateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

export function householdDate(timeZone?: string, now = new Date()): Date {
  if (!timeZone) return now
  const parts = new Intl.DateTimeFormat('en-US', {
    timeZone,
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
  }).formatToParts(now)
  const field = (name: string) =>
    Number(parts.find((part) => part.type === name)?.value)
  return new Date(field('year'), field('month') - 1, field('day'), 12)
}
