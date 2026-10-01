import { catalog, type AppState, type Product } from './data'

export function getProducts(
  state: AppState,
  includeArchived = true,
): Product[] {
  return [...catalog, ...state.customProducts]
    .filter(
      (product) =>
        includeArchived || !state.archivedProductIds?.includes(product.id),
    )
    .map((product) => {
      const current = state.productDetails?.[product.id] ?? product
      return {
        ...current,
        category: state.productGroups?.[product.id] ?? current.category,
        image: state.productImages?.[product.id] ?? current.image,
      }
    })
}

export function saveProduct(state: AppState, product: Product): AppState {
  const builtIn = catalog.some((entry) => entry.id === product.id)
  return {
    ...state,
    customProducts: builtIn
      ? state.customProducts
      : [
          ...state.customProducts.filter((entry) => entry.id !== product.id),
          product,
        ],
    productDetails: builtIn
      ? { ...state.productDetails, [product.id]: product }
      : state.productDetails,
    productGroups: { ...state.productGroups, [product.id]: product.category },
    productImages: Object.fromEntries(
      Object.entries({
        ...state.productImages,
        [product.id]: product.image,
      }).filter(([, image]) => !!image),
    ) as Record<string, string>,
    items: state.items.map((item) =>
      item.id === product.id ? { ...item, ...product } : item,
    ),
  }
}

export function archiveProduct(
  state: AppState,
  id: string,
  archived: boolean,
): AppState {
  return {
    ...state,
    archivedProductIds: [
      ...(state.archivedProductIds ?? []).filter((entry) => entry !== id),
      ...(archived ? [id] : []),
    ],
  }
}

export function deleteProduct(state: AppState, id: string): AppState {
  if (catalog.some((product) => product.id === id))
    throw new Error('Built-in items can be archived and restored.')
  if (
    [...(state.recipeBook ?? []), ...state.customRecipes].some((recipe) =>
      recipe.ingredients.some((ingredient) => ingredient.productId === id),
    )
  )
    throw new Error(
      'This item is used in a recipe. Edit its ingredients first, or archive the item instead.',
    )
  const without = <T>(values: Record<string, T> | undefined) =>
    Object.fromEntries(
      Object.entries(values ?? {}).filter(([key]) => key !== id),
    ) as Record<string, T>
  const coverage = (values: AppState['planCoverage']) =>
    Object.fromEntries(
      Object.entries(values ?? {}).map(([day, products]) => [
        day,
        without(products),
      ]),
    )
  return {
    ...state,
    customProducts: state.customProducts.filter((product) => product.id !== id),
    items: state.items.filter((item) => item.id !== id),
    productGroups: without(state.productGroups),
    productImages: without(state.productImages),
    productDetails: without(state.productDetails),
    pantry: without(state.pantry),
    archivedProductIds: (state.archivedProductIds ?? []).filter(
      (entry) => entry !== id,
    ),
    planCoverage: coverage(state.planCoverage),
    purchasedCoverage: coverage(state.purchasedCoverage),
    suggestionFeedback: Object.fromEntries(
      Object.entries(state.suggestionFeedback ?? {}).filter(
        ([key]) => key !== `restock:${id}`,
      ),
    ),
  }
}

export function deleteRecipe(state: AppState, id: string): AppState {
  const filter = <T>(entries: Record<string, T> | undefined) =>
    Object.fromEntries(
      Object.entries(entries ?? {}).filter(([day]) => state.plan[day] !== id),
    ) as Record<string, T>
  return {
    ...state,
    recipeBook: state.recipeBook?.filter((recipe) => recipe.id !== id),
    customRecipes: state.customRecipes.filter((recipe) => recipe.id !== id),
    plan: Object.fromEntries(
      Object.entries(state.plan).filter(([, recipeId]) => recipeId !== id),
    ),
    servings: filter(state.servings),
    planCoverage: filter(state.planCoverage),
    favoriteRecipeIds: state.favoriteRecipeIds.filter(
      (recipeId) => recipeId !== id,
    ),
  }
}
