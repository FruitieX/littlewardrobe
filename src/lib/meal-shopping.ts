import { getProducts } from './catalog'
import { recipes } from './data'
import type { Amount, AppState, PlanCredit, ShoppingItem } from './data'
import { dateKey, mondayOfWeek, householdDate } from './shopping'

type Requirement = PlanCredit & { mealName: string; note?: string }
const unitAliases: Record<string, [string, number]> = {
  g: ['g', 1],
  gram: ['g', 1],
  grams: ['g', 1],
  kg: ['g', 1000],
  ml: ['ml', 1],
  l: ['ml', 1000],
  litre: ['ml', 1000],
  litres: ['ml', 1000],
  liter: ['ml', 1000],
  liters: ['ml', 1000],
  clove: ['cloves', 1],
  cloves: ['cloves', 1],
  pc: ['pc', 1],
  pcs: ['pc', 1],
  piece: ['pc', 1],
  pieces: ['pc', 1],
}
function normalize(amount: Amount): Amount {
  const [unit, factor] = unitAliases[amount.unit.trim().toLowerCase()] ?? [
    amount.unit.trim().toLowerCase(),
    1,
  ]
  return { unit, quantity: amount.quantity * factor }
}
function round(quantity: number) {
  return Math.round(quantity * 10000) / 10000
}
function amounts(values: Amount[]): Amount[] {
  const totals = new Map<string, number>()
  for (const value of values) {
    const normalized = normalize(value)
    totals.set(
      normalized.unit,
      round((totals.get(normalized.unit) ?? 0) + normalized.quantity),
    )
  }
  return [...totals]
    .filter(([, quantity]) => quantity > 0)
    .map(([unit, quantity]) => ({ unit, quantity }))
}

function requirements(state: AppState, now: Date): Requirement[] {
  const today = dateKey(now)
  const end = mondayOfWeek(now, 2)
  const recipeMap = new Map(
    [...(state.recipeBook ?? recipes), ...state.customRecipes].map((recipe) => [
      recipe.id,
      recipe,
    ]),
  )
  return Object.entries(state.plan)
    .sort(([a], [b]) => a.localeCompare(b))
    .flatMap(([day, id]) => {
      const recipe = recipeMap.get(id)
      if (
        !recipe ||
        day < today ||
        day >= dateKey(end) ||
        ['cooked', 'skipped'].includes(state.mealFeedback?.[day]?.status ?? '')
      )
        return []
      const multiplier =
        (state.servings?.[day] ??
          state.preferences?.servings ??
          recipe.servings) / recipe.servings
      return recipe.ingredients.map((ingredient) => ({
        day,
        productId: ingredient.productId,
        mealName: recipe.name,
        note: ingredient.note,
        ...normalize({
          ...ingredient,
          quantity: ingredient.quantity * multiplier,
        }),
      }))
    })
}
function outstanding(state: AppState, now: Date): Requirement[] {
  // Aggregate within a day too, so duplicate recipe rows cannot each spend the same coverage.
  const grouped = new Map<string, Requirement>()
  for (const requirement of requirements(state, now)) {
    const key = JSON.stringify([
      requirement.day,
      requirement.productId,
      requirement.unit,
    ])
    const previous = grouped.get(key)
    grouped.set(key, {
      ...requirement,
      quantity: (previous?.quantity ?? 0) + requirement.quantity,
    })
  }
  const remaining = [...grouped.values()]
    .map((requirement) => {
      const covered =
        amounts(
          state.planCoverage?.[requirement.day]?.[requirement.productId] ?? [],
        ).find((amount) => amount.unit === requirement.unit)?.quantity ?? 0
      return {
        ...requirement,
        quantity: round(Math.max(0, requirement.quantity - covered)),
      }
    })
    .filter((requirement) => requirement.quantity > 0)
  const stock = new Map<string, number>()
  for (const [id, pantry] of Object.entries(state.pantry ?? {})) {
    if (pantry.expiresAt && pantry.expiresAt < dateKey(now)) continue
    if (
      !pantry.confirmed &&
      (now.getTime() - new Date(pantry.observedAt).getTime()) / 86400000 >= 7
    )
      continue
    for (const amount of amounts(pantry.amounts))
      stock.set(JSON.stringify([id, amount.unit]), amount.quantity)
  }
  for (const requirement of remaining) {
    const key = JSON.stringify([requirement.productId, requirement.unit])
    const available = stock.get(key) ?? 0
    const covered = Math.min(available, requirement.quantity)
    stock.set(key, available - covered)
    requirement.quantity = round(requirement.quantity - covered)
  }
  return remaining.filter((requirement) => requirement.quantity > 0)
}

export function preparePlanShopping(
  state: AppState,
  now = householdDate(state.preferences?.timeZone),
): AppState {
  if (state.planCoverage) return state
  let prepared: AppState = { ...state, planCoverage: {} }
  // Migrate checked groceries as limited coverage, rather than immediately asking to buy them again.
  for (const item of state.items.filter((item) => item.checked)) {
    const available = normalize(item)
    let remaining = available.quantity
    const credits = outstanding(prepared, now)
      .filter(
        (requirement) =>
          requirement.productId === item.id &&
          requirement.unit === available.unit,
      )
      .flatMap((requirement) => {
        const quantity = Math.min(remaining, requirement.quantity)
        remaining -= quantity
        return quantity > 0 ? [{ ...requirement, quantity }] : []
      })
    prepared = applyCredits(prepared, credits, 1)
    prepared = {
      ...prepared,
      items: prepared.items.map((candidate) =>
        candidate.id === item.id
          ? { ...candidate, planCredits: credits, checkedBefore: false }
          : candidate,
      ),
    }
  }
  return prepared
}

export function getShoppingItems(
  state: AppState,
  now = householdDate(state.preferences?.timeZone),
): ShoppingItem[] {
  const prepared = preparePlanShopping(state, now)
  const needed = outstanding(prepared, now)
  const all = requirements(prepared, now)
  const ids = new Set([
    ...state.items.map((item) => item.id),
    ...needed.map((requirement) => requirement.productId),
  ])
  return [...ids].flatMap<ShoppingItem>((id) => {
    const existing = state.items.find((item) => item.id === id)
    const demand = amounts(
      needed.filter((requirement) => requirement.productId === id),
    )
    const manual =
      existing && !existing.checked && existing.origin !== 'meal-plan'
        ? normalize(existing)
        : null
    const total = new Map(
      demand.map((amount) => [amount.unit, amount.quantity]),
    )
    if (manual)
      total.set(
        manual.unit,
        Math.max(manual.quantity, total.get(manual.unit) ?? 0),
      )
    if (manual)
      for (const extra of amounts(existing?.amounts ?? [])) {
        if (extra.unit !== manual.unit)
          total.set(
            extra.unit,
            Math.max(extra.quantity, total.get(extra.unit) ?? 0),
          )
      }
    const combined = [...total].map(([unit, quantity]) => ({ unit, quantity }))
    const product =
      existing ?? getProducts(state).find((product) => product.id === id)
    if (!product) return []
    if (!combined.length)
      return existing?.checked
        ? [{ ...existing, autoAdded: false, amounts: undefined }]
        : []
    const displayAmounts = combined.map((amount) =>
      manual && amount.unit === manual.unit
        ? {
            unit: existing!.unit,
            quantity: round(
              amount.quantity /
                normalize({ quantity: 1, unit: existing!.unit }).quantity,
            ),
          }
        : amount,
    )
    const primary =
      displayAmounts.find((amount) => amount.unit === existing?.unit) ??
      displayAmounts[0]
    const autoAdded = demand.some(
      (amount) =>
        amount.quantity > (manual?.unit === amount.unit ? manual.quantity : 0),
    )
    return [
      {
        ...product,
        category: state.productGroups?.[id] ?? product.category,
        ...primary,
        note:
          existing?.note ||
          [
            ...new Set(
              all
                .filter(
                  (requirement) =>
                    requirement.productId === id &&
                    needed.some(
                      (credit) =>
                        credit.productId === id &&
                        credit.day === requirement.day,
                    ),
                )
                .map((requirement) => requirement.note)
                .filter(Boolean),
            ),
          ]
            .sort()
            .join(' · '),
        image: state.productImages?.[id] || existing?.image || product.image,
        checked: false,
        amounts: displayAmounts,
        autoAdded,
        mealNames: [
          ...new Set(
            all
              .filter((requirement) => requirement.productId === id)
              .map((requirement) => requirement.mealName),
          ),
        ],
      },
    ]
  })
}

function applyCredits(
  state: AppState,
  credits: PlanCredit[],
  direction: 1 | -1,
): AppState {
  const coverage = { ...state.planCoverage }
  for (const credit of credits) {
    const day = { ...coverage[credit.day] }
    const current = amounts(day[credit.productId] ?? [])
    const normalized = normalize(credit)
    const previous = current.find((amount) => amount.unit === normalized.unit)
    day[credit.productId] = amounts([
      ...current.filter((amount) => amount.unit !== normalized.unit),
      {
        unit: normalized.unit,
        quantity: Math.max(
          0,
          (previous?.quantity ?? 0) + direction * normalized.quantity,
        ),
      },
    ])
    coverage[credit.day] = day
  }
  return { ...state, planCoverage: coverage }
}

function applyPurchasedCredits(
  state: AppState,
  credits: PlanCredit[],
  direction: -1 | 1,
): AppState {
  return {
    ...state,
    purchasedCoverage: applyCredits(
      { ...state, planCoverage: state.purchasedCoverage ?? {} },
      credits,
      direction,
    ).planCoverage,
  }
}

export function confirmPantry(
  state: AppState,
  observation: {
    productId: string
    quantity: number
    unit: string
    expiresAt: string | null
  },
  now = householdDate(state.preferences?.timeZone),
): AppState {
  const observed = normalize(observation)
  let available = observed.quantity
  const purchasedCoverage = structuredClone(state.purchasedCoverage ?? {})
  const removed: PlanCredit[] = []
  for (const day of Object.keys(purchasedCoverage).sort()) {
    if (day < dateKey(now)) continue
    const entries = purchasedCoverage[day][observation.productId]
    if (!entries) continue
    for (const amount of entries) {
      if (amount.unit !== observed.unit) continue
      const retained = Math.min(available, amount.quantity)
      available -= retained
      if (retained < amount.quantity)
        removed.push({
          day,
          productId: observation.productId,
          unit: observed.unit,
          quantity: amount.quantity - retained,
        })
      amount.quantity = retained
    }
    purchasedCoverage[day][observation.productId] = amounts(entries)
  }
  const revised = applyCredits({ ...state, purchasedCoverage }, removed, -1)
  return {
    ...revised,
    pantry: {
      ...state.pantry,
      [observation.productId]: {
        amounts:
          available > 0 ? [{ quantity: available, unit: observed.unit }] : [],
        confirmed: true,
        observedAt: new Date().toISOString(),
        expiresAt: observation.expiresAt,
        note: '',
      },
    },
  }
}

export function toggleShoppingItem(
  state: AppState,
  id: string,
  now = householdDate(state.preferences?.timeZone),
): AppState {
  const prepared = preparePlanShopping(state, now)
  const item = getShoppingItems(prepared, now).find((item) => item.id === id)
  if (!item) return state
  const stored = prepared.items.find((candidate) => candidate.id === id)
  if (item.checked) {
    const restored = applyPurchasedCredits(
      applyCredits(prepared, stored?.planCredits ?? [], -1),
      stored?.planCredits ?? [],
      -1,
    )
    restored.pantry = { ...restored.pantry }
    delete restored.pantry[id]
    const hasDemand = outstanding(restored, now).some(
      (requirement) => requirement.productId === id,
    )
    return {
      ...restored,
      items: restored.items.map((candidate) =>
        candidate.id === id
          ? {
              ...candidate,
              checked: hasDemand ? (candidate.checkedBefore ?? false) : false,
              origin: hasDemand ? candidate.origin : undefined,
              planCredits: undefined,
              checkedBefore: undefined,
            }
          : candidate,
      ),
    }
  }
  const credits = outstanding(prepared, now).filter(
    (requirement) => requirement.productId === id,
  )
  const covered = applyPurchasedCredits(
    applyCredits(prepared, credits, 1),
    credits,
    1,
  )
  const snapshot: ShoppingItem = {
    ...(stored ?? item),
    checked: true,
    origin: stored?.origin ?? (stored ? undefined : 'meal-plan'),
    planCredits: credits,
    checkedBefore: stored?.checked ?? false,
  }
  return {
    ...covered,
    items: stored
      ? covered.items.map((candidate) =>
          candidate.id === id ? snapshot : candidate,
        )
      : [...covered.items, snapshot],
    purchases: { ...covered.purchases, [id]: now.toISOString() },
  }
}

export function removeShoppingItem(
  state: AppState,
  id: string,
  now = householdDate(state.preferences?.timeZone),
): AppState {
  const prepared = preparePlanShopping(state, now)
  const skipped = applyCredits(
    prepared,
    outstanding(prepared, now).filter(
      (requirement) => requirement.productId === id,
    ),
    1,
  )
  return { ...skipped, items: skipped.items.filter((item) => item.id !== id) }
}

export function editShoppingItem(
  state: AppState,
  item: ShoppingItem,
  edits: Pick<ShoppingItem, 'category' | 'quantity' | 'unit' | 'note'>,
  now = householdDate(state.preferences?.timeZone),
): AppState {
  const changedQuantity =
    item.quantity !== edits.quantity || item.unit !== edits.unit
  const prepared = preparePlanShopping(state, now)
  // A deliberate amount edit overrides the current automatic requirement.
  // Otherwise a smaller amount would immediately be topped up again on save.
  const edited = changedQuantity
    ? applyCredits(
        prepared,
        outstanding(prepared, now).filter(
          (requirement) => requirement.productId === item.id,
        ),
        1,
      )
    : prepared
  const stored = edited.items.find((candidate) => candidate.id === item.id)
  const updated: ShoppingItem = {
    ...(stored ?? item),
    ...edits,
    checked: changedQuantity ? item.checked : (stored?.checked ?? item.checked),
    quantity: changedQuantity
      ? edits.quantity
      : (stored?.quantity ?? item.quantity),
    unit: changedQuantity ? edits.unit : (stored?.unit ?? item.unit),
    origin: changedQuantity
      ? undefined
      : (stored?.origin ?? (stored ? undefined : 'meal-plan')),
  }
  return {
    ...edited,
    items: stored
      ? edited.items.map((candidate) =>
          candidate.id === item.id ? updated : candidate,
        )
      : [...edited.items, updated],
  }
}

export function shoppingQuantity(item: ShoppingItem): string {
  return (item.amounts ?? [item])
    .map((amount) => `${Number(amount.quantity.toFixed(2))} ${amount.unit}`)
    .join(' + ')
}
