import { getProducts } from './catalog'
import { useEffect, useState } from 'react'
import { api } from './http'
import { useApp } from './state'
import type { AppState, Product, Recipe } from './data'
import type { HistoryEvent } from './api/HistoryEvent'
export type Restock = {
  product: Product
  quantity: number
  unit: string
  note: string
  intervalDays: number
  daysSince: number
  reason: string
  confidence: string
}
export type Insights = {
  restock: Restock[]
  habits: {
    productId: string
    name: string
    purchases: number
    intervalDays: number | null
    confidence: string
    preferredNote: string
    quantity: number
    unit: string
    storeId: string
  }[]
  shoppingDays: Record<string, number>
  associations: { products: string[]; daysTogether: number }[]
  purchaseCount: number
}
export function useLearning() {
  const { state, status } = useApp()
  const [insights, setInsights] = useState<Insights>({
    restock: [],
    habits: [],
    shoppingDays: {},
    associations: [],
    purchaseCount: 0,
  })
  const [history, setHistory] = useState<HistoryEvent[]>([])
  const [error, setError] = useState('')
  useEffect(() => {
    let active = true
    if (status === 'offline') return
    void Promise.all([
      api<Insights>('/insights'),
      api<HistoryEvent[]>('/history?limit=100'),
    ])
      .then(([insights, history]) => {
        if (active) {
          setInsights(insights)
          setHistory(history)
          setError('')
        }
      })
      .catch((error) => {
        if (active) setError(error.message)
      })
    return () => {
      active = false
    }
  }, [state, status])
  return { insights, history, error }
}
export function recipeScore(
  recipe: Recipe,
  state: AppState,
  now = Date.now(),
): { score: number; reasons: string[] } {
  let score = 0
  const reasons: string[] = []
  const prefs = state.preferences
  const excluded = prefs?.avoid.some(
    (term) =>
      recipe.name.toLowerCase().includes(term.toLowerCase()) ||
      recipe.ingredients.some((i) =>
        i.productId.includes(term.toLowerCase().replace(/ /g, '-')),
      ),
  )
  if (excluded)
    return { score: -1000, reasons: ['Contains an ingredient you avoid'] }
  const products = recipe.ingredients
    .map((i) => getProducts(state).find((p) => p.id === i.productId))
    .filter((p): p is Product => !!p)
  const dietary = prefs?.dietary.map((d) => d.toLowerCase()) ?? []
  const incompatible =
    (dietary.some((d) => ['vegetarian', 'vegan'].includes(d)) &&
      products.some((p) => ['Meat', 'Fish'].includes(p.category))) ||
    (dietary.includes('vegan') &&
      products.some(
        (p) =>
          ['Cheese', 'Milk & eggs'].includes(p.category) || p.id === 'honey',
      )) ||
    (dietary.includes('gluten-free') &&
      products.some(
        (p) =>
          /pasta|bread|flour|bulgur|couscous|barley|tortilla/i.test(p.name) &&
          !p.name.toLowerCase().includes('gluten-free'),
      ))
  if (incompatible)
    return {
      score: -1000,
      reasons: ['Does not match your dietary preferences'],
    }
  if (state.favoriteRecipeIds.includes(recipe.id)) {
    score += 3
    reasons.push('A family favorite')
  }
  const meals = Object.entries(state.mealFeedback ?? {}).filter(
    ([, f]) => f.recipeId === recipe.id,
  )
  const cooked = meals
    .filter(([, f]) => f.status === 'cooked')
    .sort(([a], [b]) => b.localeCompare(a))
  if (cooked[0]) {
    const ago = Math.floor((now - new Date(cooked[0][0]).getTime()) / 86400000)
    score += Math.min(ago / 7, 4) - (ago < 7 ? 6 : 0)
    reasons.push(ago < 7 ? 'Cooked recently' : `Last cooked ${ago} days ago`)
  } else {
    score += (prefs?.novelty ?? 0.25) * 4
    reasons.push('Something to try from your recipe book')
  }
  const rating = cooked
    .map(([, f]) => f.rating)
    .filter((r): r is number => r !== null)
  if (rating.length) {
    score += rating.reduce((a, b) => a + b, 0) / rating.length - 3
    reasons.push(
      `Family rating ${(rating.reduce((a, b) => a + b, 0) / rating.length).toFixed(1)}/5`,
    )
  }
  score -= meals.filter(([, f]) => f.status === 'skipped').length
  const shared = recipe.ingredients.filter(
    (i) =>
      state.items.some((item) => item.id === i.productId && !item.checked) ||
      state.pantry?.[i.productId]?.amounts.length,
  ).length
  if (shared) {
    score += (shared / recipe.ingredients.length) * 5
    reasons.push(`${shared} ingredients already listed or likely available`)
  }
  if (recipe.minutes <= (prefs?.maxWeekdayMinutes ?? 30)) {
    score += 2
    reasons.push(
      `Fits your ${prefs?.maxWeekdayMinutes ?? 30}-minute weekday limit`,
    )
  }
  return { score, reasons }
}
export function suggestWeek(
  recipes: Recipe[],
  state: AppState,
  rotation = 0,
): Recipe[] {
  const ranked = recipes
    .filter((r) => recipeScore(r, state).score > -999)
    .sort((a, b) => recipeScore(b, state).score - recipeScore(a, state).score)
  if (!ranked.length) return []
  return Array.from(
    { length: 7 },
    (_, i) => ranked[(i + rotation) % ranked.length],
  )
}
export function discovery(recipes: Recipe[], state: AppState): Recipe | null {
  const base = [...recipes]
    .filter((r) => recipeScore(r, state).score > -999)
    .sort(
      (a, b) => recipeScore(b, state).score - recipeScore(a, state).score,
    )[0]
  if (!base) return null
  const choices = [
    { id: 'chickpeas', name: 'chickpea', unit: 'g', quantity: 240 },
    { id: 'spinach', name: 'spinach', unit: 'g', quantity: 150 },
    { id: 'lemon', name: 'lemon', unit: 'pc', quantity: 1 },
  ]
  const extra = choices.find(
    (p) =>
      !base.ingredients.some((i) => i.productId === p.id) &&
      !state.preferences?.avoid.some((a) => p.id.includes(a.toLowerCase())),
  )
  if (!extra) return null
  const id = `discovery-${base.id}-${extra.id}`
  if (
    recipes.some((r) => r.id === id) ||
    state.suggestionFeedback?.[id]?.status === 'dismissed'
  )
    return null
  return {
    ...base,
    id,
    name: `${base.name} with ${extra.name}`,
    subtitle: `A small variation on ${base.name}. Taste and adjust the addition for your family.`,
    image: '/images/recipe-placeholder.svg',
    tag: 'New variation',
    tags: [...(base.tags ?? []), 'Variation'],
    ingredients: [
      ...base.ingredients,
      { productId: extra.id, quantity: extra.quantity, unit: extra.unit },
    ],
    steps: [
      ...base.steps,
      `Add the ${extra.name} to taste: rinse and warm chickpeas, wilt spinach, or finish with lemon juice, as appropriate. Taste before serving.`,
    ],
  }
}
