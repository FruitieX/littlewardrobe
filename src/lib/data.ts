export const categories = [
  'Fruits',
  'Vegetables',
  'Milk & eggs',
  'Cheese',
  'Meat',
  'Fish',
  'Ingredients',
  'Spices',
  'Grain products',
  'Snacks',
  'Sweets',
  'Beverages',
  'Household',
  'Care',
  'Health',
  'Pet supplies',
  'Home',
  'Garden',
  'Bread',
  'Pastries',
  'Frozen',
  'Convenience',
] as const
export type Category = string
export type Product = {
  id: string
  name: string
  category: Category
  icon: string
  image?: string
  aliases?: string[]
  interval?: number
}
export type ShoppingItem = Product & {
  quantity: number
  unit: string
  note: string
  checked: boolean
  origin?: 'meal-plan'
  planCredits?: PlanCredit[]
  checkedBefore?: boolean
  amounts?: Amount[]
  autoAdded?: boolean
  mealNames?: string[]
  manualAmount?: Amount
}
export type Amount = { quantity: number; unit: string }
export type PlanCredit = Amount & { day: string; productId: string }
export type PlanCoverage = Record<string, Record<string, Amount[]>>
export type Ingredient = {
  productId: string
  quantity: number
  unit: string
  note?: string
}
export type Recipe = {
  id: string
  name: string
  subtitle: string
  image: string
  minutes: number
  tag: string
  servings: number
  ingredients: Ingredient[]
  steps: string[]
  favorite?: boolean
  tags?: string[]
}
export type Store = {
  id: string
  name: string
  location: string
  order: Category[]
  image?: string
}
export type Plan = Record<string, string>
export type AppState = {
  garments?: import('./api/Garment').Garment[]
  children?: import('./api/Child').Child[]
  locations?: import('./api/StorageLocation').StorageLocation[]
  saleListings?: import('./api/SaleListing').SaleListing[]
  items: ShoppingItem[]
  stores: Store[]
  storeId: string
  plan: Plan
  purchases: Record<string, string>
  customProducts: Product[]
  customRecipes: Recipe[]
  favoriteRecipeIds: string[]
  planCoverage?: PlanCoverage
  recipeBook?: Recipe[]
  groups?: string[]
  pantry?: Record<string, import('./api/PantryItem').PantryItem>
  servings?: Record<string, number>
  mealFeedback?: Record<string, import('./api/MealFeedback').MealFeedback>
  suggestionFeedback?: Record<
    string,
    import('./api/SuggestionFeedback').SuggestionFeedback
  >
  preferences?: import('./api/Preferences').Preferences
  productGroups?: Record<string, string>
  productImages?: Record<string, string>
  productDetails?: Record<string, Product>
  archivedProductIds?: string[]
  purchasedCoverage?: PlanCoverage
}

const productGroups: [Category, string, string[]][] = [
  [
    'Fruit & vegetables',
    '🥬',
    [
      'Avocado',
      'Broccoli',
      'Cucumber',
      'Cherry tomatoes',
      'Carrots',
      'Spinach',
      'Potatoes',
      'Sweet potatoes',
      'Red onion',
      'Onion',
      'Garlic',
      'Bell pepper',
      'Zucchini',
      'Mushrooms',
      'Lettuce',
      'Cauliflower',
      'Peas',
      'Cabbage',
      'Celery',
      'Asparagus',
      'Beetroot',
      'Pumpkin',
      'Bananas',
      'Apples',
      'Lemon',
      'Lime',
      'Oranges',
      'Strawberries',
      'Blueberries',
      'Raspberries',
      'Grapes',
      'Pears',
      'Mango',
      'Kiwi',
      'Watermelon',
      'Fresh basil',
      'Parsley',
      'Dill',
    ],
  ],
  [
    'Bread & pastries',
    '🥖',
    [
      'Sourdough bread',
      'Rye bread',
      'Toast bread',
      'Bread rolls',
      'Bagels',
      'Croissants',
      'Tortillas',
      'Pita bread',
      'Cinnamon buns',
    ],
  ],
  [
    'Dairy & eggs',
    '🥛',
    [
      'Oat milk',
      'Milk',
      'Greek yogurt',
      'Yogurt',
      'Eggs',
      'Butter',
      'Cream',
      'Sour cream',
      'Cottage cheese',
      'Cheddar',
      'Mozzarella',
      'Parmesan',
      'Feta',
      'Cream cheese',
    ],
  ],
  [
    'Meat & fish',
    '🐟',
    [
      'Salmon',
      'Chicken breast',
      'Chicken thighs',
      'Minced beef',
      'Minced turkey',
      'Pork chops',
      'Bacon',
      'Sausages',
      'Ham',
      'Tuna',
      'Cod',
      'Prawns',
      'Smoked salmon',
    ],
  ],
  [
    'Grains & pasta',
    '🍝',
    [
      'Pasta',
      'Rice',
      'Couscous',
      'Quinoa',
      'Oats',
      'Flour',
      'Noodles',
      'Granola',
      'Cereal',
      'Breadcrumbs',
      'Barley',
      'Semolina',
    ],
  ],
  [
    'Ingredients & spices',
    '🫙',
    [
      'Chickpeas',
      'Lentils',
      'Black beans',
      'Kidney beans',
      'Chopped tomatoes',
      'Tomato paste',
      'Coconut milk',
      'Vegetable stock',
      'Chicken stock',
      'Olive oil',
      'Rapeseed oil',
      'Soy sauce',
      'Vinegar',
      'Honey',
      'Sugar',
      'Salt',
      'Black pepper',
      'Paprika',
      'Cumin',
      'Cinnamon',
      'Curry powder',
      'Oregano',
      'Baking powder',
      'Yeast',
      'Peanut butter',
      'Jam',
      'Pesto',
      'Mustard',
      'Ketchup',
      'Mayonnaise',
    ],
  ],
  [
    'Frozen',
    '🧊',
    [
      'Frozen peas',
      'Frozen berries',
      'Frozen vegetables',
      'Ice cream',
      'Frozen pizza',
      'Fish fingers',
      'Frozen spinach',
    ],
  ],
  [
    'Convenience',
    '🥡',
    [
      'Ready-made soup',
      'Dumplings',
      'Falafel',
      'Tofu',
      'Hummus',
      'Pizza dough',
      'Ready-made salad',
    ],
  ],
  [
    'Snacks & sweets',
    '🍿',
    [
      'Chocolate',
      'Crisps',
      'Popcorn',
      'Crackers',
      'Biscuits',
      'Nuts',
      'Dried fruit',
      'Candy',
      'Rice cakes',
      'Protein bars',
    ],
  ],
  [
    'Beverages',
    '☕',
    [
      'Coffee',
      'Tea',
      'Sparkling water',
      'Juice',
      'Lemonade',
      'Hot chocolate',
      'Mineral water',
    ],
  ],
  [
    'Household',
    '🧻',
    [
      'Toilet paper',
      'Kitchen paper',
      'Dish soap',
      'Dishwasher tablets',
      'Laundry detergent',
      'Bin bags',
      'Sponges',
      'Baking paper',
      'Aluminium foil',
      'Cleaning spray',
    ],
  ],
  [
    'Care & health',
    '🧴',
    [
      'Shampoo',
      'Conditioner',
      'Toothpaste',
      'Toothbrush',
      'Hand soap',
      'Shower gel',
      'Deodorant',
      'Sunscreen',
      'Plasters',
      'Vitamins',
      'Tissues',
      'Nappies',
    ],
  ],
  ['Pets', '🐾', ['Cat food', 'Dog food', 'Cat litter', 'Pet treats']],
  [
    'Home & garden',
    '🪴',
    ['Batteries', 'Light bulbs', 'Candles', 'Potting soil', 'Plant food'],
  ],
]
productGroups.push(
  ...([
    [
      'Fruits',
      '🍎',
      [
        'Apricots',
        'Peaches',
        'Nectarines',
        'Plums',
        'Pineapple',
        'Clementines',
        'Cherries',
        'Dates',
        'Figs',
        'Pomegranate',
        'Grapefruit',
      ],
    ],
    [
      'Vegetables',
      '🥬',
      [
        'Leeks',
        'Kale',
        'Radishes',
        'Turnips',
        'Parsnips',
        'Green beans',
        'Aubergine',
        'Brussels sprouts',
        'Pak choi',
        'Spring onions',
        'Artichokes',
        'Chilli peppers',
        'Fennel',
        'Butternut squash',
        'Coriander',
      ],
    ],
    [
      'Milk & eggs',
      '🛒',
      [
        'Whole milk',
        'Skimmed milk',
        'Lactose-free milk',
        'Soy milk',
        'Almond milk',
        'Coconut yogurt',
        'Crème fraîche',
        'Sour cream',
        'Quark',
        'Cottage cheese',
        'Kefir',
      ],
    ],
    [
      'Cheese',
      '🧀',
      [
        'Halloumi',
        'Gouda',
        'Brie',
        'Goat cheese',
        'Ricotta',
        'Cream cheese',
        'Emmental',
        'Blue cheese',
        'Vegan cheese',
      ],
    ],
    [
      'Meat',
      '🥩',
      [
        'Turkey',
        'Lamb',
        'Pork chops',
        'Beef steak',
        'Sausages',
        'Ham',
        'Chicken thighs',
        'Meatballs',
      ],
    ],
    [
      'Fish',
      '🐟',
      [
        'Cod',
        'Tuna steak',
        'Prawns',
        'Smoked salmon',
        'Herring',
        'Mussels',
        'Fish fingers',
      ],
    ],
    [
      'Ingredients',
      '🛒',
      [
        'Red lentils',
        'Green lentils',
        'White beans',
        'Kidney beans',
        'Black beans',
        'Tofu',
        'Tempeh',
        'Coconut cream',
        'Tahini',
        'Sesame oil',
        'Balsamic vinegar',
        'Rice vinegar',
        'Tomato paste',
        'Passata',
        'Capers',
        'Olives',
        'Mustard',
        'Ketchup',
        'Mayonnaise',
        'Breadcrumbs',
        'Cornflour',
        'Gelatine',
        'Yeast',
        'Vanilla extract',
      ],
    ],
    [
      'Spices',
      '🧂',
      [
        'Turmeric',
        'Cinnamon',
        'Cardamom',
        'Nutmeg',
        'Cayenne pepper',
        'Chilli flakes',
        'Curry powder',
        'Coriander seeds',
        'Dried basil',
        'Rosemary',
        'Thyme',
        'Bay leaves',
        'Allspice',
        'Star anise',
        'Smoked paprika',
      ],
    ],
    [
      'Grain products',
      '🛒',
      [
        'Bulgur',
        'Couscous',
        'Barley',
        'Polenta',
        'Buckwheat',
        'Wholegrain pasta',
        'Rice noodles',
        'Lasagne sheets',
        'Semolina',
        'Rye flour',
        'Granola',
        'Muesli',
      ],
    ],
    [
      'Snacks',
      '🛒',
      [
        'Popcorn',
        'Rice cakes',
        'Pretzels',
        'Hummus',
        'Trail mix',
        'Protein bars',
      ],
    ],
    [
      'Sweets',
      '🛒',
      [
        'Gummy sweets',
        'Liquorice',
        'Marshmallows',
        'Dark chocolate',
        'Biscuits',
        'Wafer biscuits',
      ],
    ],
    [
      'Beverages',
      '🛒',
      [
        'Sparkling water',
        'Apple juice',
        'Orange juice',
        'Herbal tea',
        'Black tea',
        'Green tea',
        'Cocoa powder',
        'Lemonade',
        'Alcohol-free beer',
      ],
    ],
    [
      'Household',
      '🛒',
      [
        'Laundry powder',
        'Fabric softener',
        'Dishwasher salt',
        'Rinse aid',
        'Floor cleaner',
        'Toilet cleaner',
        'Glass cleaner',
        'Rubber gloves',
        'Bin liners',
        'Microfibre cloths',
      ],
    ],
    [
      'Care',
      '🛒',
      [
        'Lip balm',
        'Hand cream',
        'Body lotion',
        'Face wash',
        'Razors',
        'Shaving foam',
        'Cotton pads',
        'Sanitary pads',
        'Tampons',
      ],
    ],
    [
      'Health',
      '🛒',
      [
        'Pain relief',
        'Thermometer',
        'First aid tape',
        'Disinfectant',
        'Cough drops',
        'Contact lens solution',
      ],
    ],
    [
      'Pet supplies',
      '🐾',
      ['Dog treats', 'Cat treats', 'Bird seed', 'Rabbit food', 'Pet shampoo'],
    ],
    ['Home', '🪴', ['Matches', 'LED bulbs', 'Storage boxes', 'Kitchen towels']],
    ['Garden', '🪴', ['Seeds', 'Compost', 'Garden gloves', 'Plant pots']],
    [
      'Bread',
      '🥖',
      [
        'Baguette',
        'Flatbread',
        'Crispbread',
        'Wholegrain bread',
        'Gluten-free bread',
      ],
    ],
    [
      'Pastries',
      '🥐',
      ['Muffins', 'Doughnuts', 'Scones', 'Puff pastry', 'Shortcrust pastry'],
    ],
    [
      'Frozen',
      '🛒',
      [
        'Frozen peas',
        'Frozen spinach',
        'Frozen berries',
        'Frozen mango',
        'Frozen chips',
        'Frozen dumplings',
        'Ice lollies',
      ],
    ],
    [
      'Convenience',
      '🛒',
      [
        'Ready-made soup',
        'Ready-made salad',
        'Sandwiches',
        'Instant noodles',
        'Vegetable burgers',
        'Falafel',
      ],
    ],
  ] as [Category, string, string[]][]),
)
const fruits = new Set([
  'Bananas',
  'Apples',
  'Lemon',
  'Lime',
  'Oranges',
  'Strawberries',
  'Blueberries',
  'Raspberries',
  'Grapes',
  'Pears',
  'Mango',
  'Kiwi',
  'Watermelon',
])
function detailedGroup(name: string, group: string): string {
  if (group === 'Fruit & vegetables')
    return fruits.has(name) ? 'Fruits' : 'Vegetables'
  if (group === 'Dairy & eggs')
    return /cheddar|feta|parmesan|mozzarella/i.test(name)
      ? 'Cheese'
      : 'Milk & eggs'
  if (group === 'Bread & pastries')
    return /croissant|bun/i.test(name) ? 'Pastries' : 'Bread'
  if (group === 'Meat & fish')
    return /salmon|tuna|fish/i.test(name) ? 'Fish' : 'Meat'
  if (group === 'Ingredients & spices')
    return /salt|pepper|paprika|cumin|oregano/i.test(name)
      ? 'Spices'
      : 'Ingredients'
  if (group === 'Grains & pasta') return 'Grain products'
  if (group === 'Snacks & sweets')
    return /chocolate|sweet|cookie/i.test(name) ? 'Sweets' : 'Snacks'
  if (group === 'Care & health')
    return /plaster|vitamin/i.test(name) ? 'Health' : 'Care'
  if (group === 'Pets') return 'Pet supplies'
  if (group === 'Home & garden')
    return /soil|plant/i.test(name) ? 'Garden' : 'Home'
  return group
}
const icons: Record<string, string> = {
  Avocado: '🥑',
  Broccoli: '🥦',
  Cucumber: '🥒',
  'Cherry tomatoes': '🍅',
  Carrots: '🥕',
  Potatoes: '🥔',
  Bananas: '🍌',
  Apples: '🍎',
  Lemon: '🍋',
  Oranges: '🍊',
  Strawberries: '🍓',
  Blueberries: '🫐',
  Grapes: '🍇',
  'Bell pepper': '🫑',
  Garlic: '🧄',
  Onion: '🧅',
  'Red onion': '🧅',
  Mushrooms: '🍄',
  Eggs: '🥚',
  Butter: '🧈',
  Cheddar: '🧀',
  Feta: '🧀',
  Parmesan: '🧀',
  Mozzarella: '🧀',
  'Oat milk': '🥛',
  'Sourdough bread': '🍞',
  'Rye bread': '🍞',
  Rice: '🍚',
  Honey: '🍯',
  Chicken: '🍗',
  'Chicken breast': '🍗',
  Bacon: '🥓',
  'Minced beef': '🥩',
  'Greek yogurt': '🥣',
  'Olive oil': '🫒',
  Chocolate: '🍫',
  Coffee: '☕',
  'Toilet paper': '🧻',
  'Dish soap': '🧴',
}
export const productId = (name: string) =>
  name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
const groceryAliases: Record<string, string[]> = {
  'Oat milk': ['kauramaito'],
  Milk: ['maito'],
  Bananas: ['banaani'],
  Apples: ['omena'],
  Eggs: ['kananmuna'],
  'Bread rolls': ['sämpylä'],
  'Rye bread': ['ruisleipä'],
  Coffee: ['kahvi'],
  Potatoes: ['peruna'],
  Carrots: ['porkkana'],
  'Chicken breast': ['kana'],
  Rice: ['riisi'],
  'Toilet paper': ['wc-paperi'],
}
export const catalog: Product[] = productGroups
  .flatMap(([category, icon, names]) =>
    names.map((name) => ({
      id: productId(name),
      name,
      category: detailedGroup(name, category),
      aliases: groceryAliases[name] ?? [],
      icon: icons[name] ?? icon,
    })),
  )
  // Keep shared names represented by a single catalog identity.
  .filter((p, i, list) => list.findIndex((x) => x.id === p.id) === i)
export const findProduct = (id: string): Product =>
  catalog.find((p) => p.id === id) ?? catalog[0]

export const recipes: Recipe[] = [
  {
    id: 'green-pasta',
    name: 'Green goddess pasta',
    subtitle: 'A little green, a lot of goodness.',
    image: '/images/pasta.jpg',
    minutes: 20,
    tag: 'Vegetarian',
    servings: 4,
    favorite: true,
    ingredients: [
      { productId: 'pasta', quantity: 400, unit: 'g' },
      { productId: 'spinach', quantity: 150, unit: 'g' },
      { productId: 'fresh-basil', quantity: 1, unit: 'bunch' },
      { productId: 'parmesan', quantity: 80, unit: 'g' },
      { productId: 'garlic', quantity: 2, unit: 'cloves' },
      { productId: 'olive-oil', quantity: 3, unit: 'tbsp' },
      { productId: 'lemon', quantity: 1, unit: 'pc' },
    ],
    steps: [
      'Cook the pasta in salted water. Save a cup of the pasta water before draining.',
      'Blend spinach, basil, garlic, olive oil, lemon juice, and half the Parmesan until smooth.',
      'Toss with the warm pasta, adding a splash of pasta water to make it silky. Finish with the remaining Parmesan.',
    ],
  },
  {
    id: 'salmon-bowl',
    name: 'Lemon salmon bowls',
    subtitle: 'Bright, fresh, and a family favorite.',
    image: '/images/salmon.jpg',
    minutes: 30,
    tag: 'Family favorite',
    servings: 4,
    favorite: true,
    ingredients: [
      { productId: 'salmon', quantity: 600, unit: 'g' },
      { productId: 'rice', quantity: 300, unit: 'g' },
      { productId: 'avocado', quantity: 2, unit: 'pc' },
      { productId: 'cucumber', quantity: 1, unit: 'pc' },
      { productId: 'lemon', quantity: 1, unit: 'pc' },
    ],
    steps: [
      'Cook the rice. Heat the oven to 200°C.',
      'Season the salmon with lemon and bake for 15–18 minutes, until cooked through.',
      'Slice the vegetables. Serve the salmon over rice with avocado, cucumber, and a squeeze of lemon.',
    ],
  },
  {
    id: 'rainbow-salad',
    name: 'Rainbow crunch bowls',
    subtitle: 'Colorful bowls, happy little eaters.',
    image: '/images/salad.jpg',
    minutes: 15,
    tag: 'Vegetarian',
    servings: 4,
    ingredients: [
      { productId: 'chickpeas', quantity: 400, unit: 'g' },
      { productId: 'cherry-tomatoes', quantity: 250, unit: 'g' },
      { productId: 'cucumber', quantity: 1, unit: 'pc' },
      { productId: 'feta', quantity: 150, unit: 'g' },
      { productId: 'olive-oil', quantity: 2, unit: 'tbsp' },
    ],
    steps: [
      'Rinse the chickpeas and chop the vegetables.',
      'Divide between bowls and crumble over the feta.',
      'Drizzle with olive oil, season, and serve. Let everyone build their own bowl.',
    ],
  },
  {
    id: 'tomato-soup',
    name: 'Cozy tomato soup',
    subtitle: 'The comfort-food kind of afternoon.',
    image: '/images/soup.jpg',
    minutes: 25,
    tag: 'Vegetarian',
    servings: 4,
    ingredients: [
      { productId: 'chopped-tomatoes', quantity: 800, unit: 'g' },
      { productId: 'onion', quantity: 1, unit: 'pc' },
      { productId: 'garlic', quantity: 2, unit: 'cloves' },
      { productId: 'vegetable-stock', quantity: 500, unit: 'ml' },
      { productId: 'cream', quantity: 100, unit: 'ml' },
      { productId: 'sourdough-bread', quantity: 1, unit: 'loaf' },
    ],
    steps: [
      'Soften the onion and garlic in a little oil.',
      'Add tomatoes and stock. Simmer for 20 minutes.',
      'Blend until smooth, stir in cream, and serve with toasted sourdough.',
    ],
  },
  {
    id: 'taco-night',
    name: 'Build-your-own tacos',
    subtitle: 'A lunch everyone gets to make their own.',
    image: '/images/tacos.jpg',
    minutes: 25,
    tag: 'Family favorite',
    servings: 4,
    ingredients: [
      { productId: 'tortillas', quantity: 8, unit: 'pc' },
      { productId: 'black-beans', quantity: 400, unit: 'g' },
      { productId: 'avocado', quantity: 2, unit: 'pc' },
      { productId: 'cherry-tomatoes', quantity: 250, unit: 'g' },
      { productId: 'cheddar', quantity: 100, unit: 'g' },
    ],
    steps: [
      'Warm the beans with cumin and paprika.',
      'Chop tomatoes and avocado, grate the cheese, and warm the tortillas.',
      'Put everything on the table and let everyone assemble their tacos.',
    ],
  },
]

export function initialState(): AppState {
  const seeds: [string, number, string, string, boolean][] = [
    ['avocado', 2, 'pc', 'Ripe, ready for lunch', false],
    ['cherry-tomatoes', 250, 'g', 'The sweet little ones', false],
    ['broccoli', 1, 'pc', '', false],
    ['bananas', 6, 'pc', 'A little green is fine', false],
    ['oat-milk', 2, 'L', 'Oatly · barista', false],
    ['eggs', 1, 'box', 'Free-range · 10 pack', false],
    ['greek-yogurt', 500, 'g', 'Plain, full-fat', false],
    ['parmesan', 80, 'g', 'For Wednesday’s pasta', false],
    ['sourdough-bread', 1, 'loaf', 'Sliced, please', false],
    ['pasta', 400, 'g', 'Rigatoni or penne', false],
    ['apples', 4, 'pc', 'Gala', true],
    ['cucumber', 1, 'pc', '', true],
    ['coffee', 1, 'bag', 'Medium roast', true],
  ]
  const now = new Date()
  const ago = (days: number) =>
    new Date(now.getTime() - days * 86400000).toISOString()
  const monday = new Date(now)
  monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7))
  const plan: Plan = {}
  ;[
    'salmon-bowl',
    'rainbow-salad',
    'green-pasta',
    'tomato-soup',
    'taco-night',
  ].forEach((id, index) => {
    const day = new Date(monday)
    day.setDate(day.getDate() + index)
    const key = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, '0')}-${String(day.getDate()).padStart(2, '0')}`
    plan[key] = id
  })
  return {
    items: seeds.map(([id, quantity, unit, note, checked]) => ({
      ...findProduct(id),
      quantity,
      unit,
      note,
      checked,
    })),
    stores: [
      {
        id: 'prisma',
        name: 'Prisma',
        location: 'Our weekly shop',
        order: [...categories],
      },
      {
        id: 'lidl',
        name: 'Lidl',
        location: 'A quick top-up',
        order: [
          'Bread & pastries',
          'Fruit & vegetables',
          'Meat & fish',
          'Dairy & eggs',
          ...categories.filter(
            (c) =>
              ![
                'Bread & pastries',
                'Fruit & vegetables',
                'Meat & fish',
                'Dairy & eggs',
              ].includes(c),
          ),
        ],
      },
      {
        id: 'k-market',
        name: 'K-Market',
        location: 'Around the corner',
        order: [
          'Milk & eggs',
          ...categories.filter((c) => c !== 'Milk & eggs'),
        ],
      },
    ],
    storeId: 'prisma',
    plan,
    customProducts: [],
    customRecipes: [],
    favoriteRecipeIds: ['green-pasta', 'salmon-bowl'],
    purchases: {
      'oat-milk': ago(8),
      bananas: ago(7),
      eggs: ago(9),
      'bread-rolls': ago(9),
      coffee: ago(1),
      cucumber: ago(1),
      apples: ago(1),
    },
  }
}
