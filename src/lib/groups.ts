import { getProducts } from './catalog'
import { categories, type AppState } from './data'

/** Move every category reference in one household operation, retaining aisle order. */
export function changeGroup(
  state: AppState,
  source: string,
  target: string,
  remove = false,
): AppState {
  const groups = state.groups ?? [...categories]
  if (!groups.includes(source) || !target.trim())
    throw new Error('Choose an existing group and a nonempty name.')
  if (
    remove
      ? source === target || !groups.includes(target)
      : target !== source &&
        groups.some((group) => group.toLowerCase() === target.toLowerCase())
  )
    throw new Error(
      remove
        ? 'Choose another group for these items.'
        : 'A group with that name already exists.',
    )
  const productGroups = { ...state.productGroups }
  for (const product of getProducts(state))
    if ((productGroups[product.id] ?? product.category) === source)
      productGroups[product.id] = target
  return {
    ...state,
    groups: remove
      ? groups.filter((group) => group !== source)
      : groups.map((group) => (group === source ? target : group)),
    productGroups,
    productDetails: Object.fromEntries(
      Object.entries(state.productDetails ?? {}).map(([id, product]) => [
        id,
        product.category === source
          ? { ...product, category: target }
          : product,
      ]),
    ),
    customProducts: state.customProducts.map((product) =>
      product.category === source ? { ...product, category: target } : product,
    ),
    items: state.items.map((item) =>
      item.category === source ? { ...item, category: target } : item,
    ),
    stores: state.stores.map((store) => ({
      ...store,
      order: remove
        ? store.order.filter((group) => group !== source)
        : store.order.map((group) => (group === source ? target : group)),
    })),
  }
}
