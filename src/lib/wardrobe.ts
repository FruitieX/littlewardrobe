import type { Garment } from './api/Garment'
import type { SaleListing } from './api/SaleListing'
import type { Child } from './api/Child'
import type { Change } from './api/Change'
export type { Garment, SaleListing, Child }
export function recordChanges<T extends { id: string }>(
  collection: string,
  before: T | null,
  after: T,
): Change[] {
  const value = (v: unknown): Change['after'] =>
    JSON.parse(JSON.stringify(v ?? null))
  if (!before)
    return [
      {
        collection,
        key: after.id,
        field: null,
        before: null,
        after: value(after),
      },
    ]
  return Object.keys(after)
    .filter(
      (k) =>
        k !== 'id' &&
        JSON.stringify(before[k as keyof T]) !==
          JSON.stringify(after[k as keyof T]),
    )
    .map((field) => ({
      collection,
      key: after.id,
      field,
      before: value(before[field as keyof T]),
      after: value(after[field as keyof T]),
    }))
}
export const categories = {
  tops: 'Tops',
  bottoms: 'Bottoms',
  'one-pieces': 'One-pieces',
  knitwear: 'Knitwear',
  outerwear: 'Outerwear',
  sleepwear: 'Sleepwear',
  shoes: 'Shoes',
  accessories: 'Accessories',
  other: 'Other',
}
export const statuses = {
  active: 'In rotation',
  stored: 'Saved for later',
  sell: 'Ready to sell',
  reserved: 'Reserved',
  sold: 'Sold',
  donated: 'Donated',
  keepsake: 'Keepsake',
}
export const conditions = {
  unknown: 'Needs checking',
  new: 'New with tags',
  excellent: 'Excellent',
  good: 'Good',
  fair: 'Fair',
  repair: 'Needs repair',
}
export const blankGarment = (): Garment => ({
  id: crypto.randomUUID(),
  name: '',
  category: 'tops',
  brand: '',
  size: '',
  sizeSystem: 'cm',
  color: '',
  material: '',
  season: 'all',
  condition: 'unknown',
  defects: '',
  status: 'stored',
  childId: '',
  nextChildId: '',
  locationId: '',
  purchasePrice: null,
  purchaseDate: '',
  purchaseSource: '',
  notes: '',
  photos: [],
  createdAt: new Date().toISOString(),
})
export function age(birthday: string) {
  if (!birthday) return ''
  const born = new Date(birthday + 'T12:00:00'),
    today = new Date()
  let months =
    (today.getFullYear() - born.getFullYear()) * 12 +
    today.getMonth() -
    born.getMonth()
  if (today.getDate() < born.getDate()) months--
  if (months < 0) return ''
  return months < 24 ? `${months} months` : `${Math.floor(months / 12)} years`
}
export const euro = (n: number) =>
  new Intl.NumberFormat('fi-FI', { style: 'currency', currency: 'EUR' }).format(
    n,
  )
export function fits(
  g: Garment,
  c: Child,
): 'small' | 'now' | 'later' | 'unknown' {
  const size = g.sizeSystem === 'eu-shoe' ? c.shoeSize : c.clothingSize
  if (
    !['cm', 'eu-shoe'].includes(g.sizeSystem) ||
    !/^\d{2,3}(?:\s*[-/]\s*\d{2,3})?$/.test(g.size) ||
    !/^\d{2,3}$/.test(size)
  )
    return 'unknown'
  const values = g.size.split(/[-/]/).map(Number),
    current = Number(size)
  return current > Math.max(...values)
    ? 'small'
    : current < Math.min(...values)
      ? 'later'
      : 'now'
}
const fiCategory: Record<string, string> = {
  tops: 'Paita',
  bottoms: 'Housut',
  'one-pieces': 'Body / haalari',
  knitwear: 'Neule',
  outerwear: 'Ulkovaatteet',
  sleepwear: 'Yöasu',
  shoes: 'Kengät',
  accessories: 'Asuste',
  other: 'Lastenvaate',
}
const fiCondition: Record<string, string> = {
  unknown: 'Kunto tarkistettava',
  new: 'Uusi, hintalaput paikoillaan',
  excellent: 'Erinomainen',
  good: 'Hyvä',
  fair: 'Tyydyttävä',
  repair: 'Korjattavaa',
}
export function manualListing(items: Garment[], platform: string): SaleListing {
  return {
    id: crypto.randomUUID(),
    garmentIds: items.map((g) => g.id),
    platform,
    title:
      items.length > 1
        ? `Lastenvaatepaketti, ${items.length} kpl`
        : [items[0]?.brand, fiCategory[items[0]?.category], items[0]?.size]
            .filter(Boolean)
            .join(' '),
    description: items
      .map((g) =>
        [
          `${g.brand ? g.brand + ' ' : ''}${fiCategory[g.category]}, koko ${g.size || 'tarkistettava'}.`,
          `Kunto: ${fiCondition[g.condition]}.`,
          g.defects ? `Huomioitavaa: ${g.defects}` : '',
        ]
          .filter(Boolean)
          .join('\n'),
      )
      .join('\n\n'),
    price: 0,
    priceReason:
      'Set your asking price. No market comparison has been performed.',
    photos: items
      .flatMap((g) =>
        g.photos
          .filter((p) => !['tag', 'enhanced'].includes(p.kind))
          .map((p) => p.url),
      )
      .slice(0, 10),
    status: 'draft',
    externalUrl: '',
    soldPrice: null,
    soldAt: '',
    createdAt: new Date().toISOString(),
  }
}
export function resetApprovals(
  listings: SaleListing[],
  old: Garment | undefined,
  next: Garment,
) {
  if (
    !old ||
    [
      'name',
      'brand',
      'size',
      'sizeSystem',
      'color',
      'material',
      'season',
      'condition',
      'defects',
      'photos',
    ].every(
      (k) =>
        JSON.stringify(old[k as keyof Garment]) ===
        JSON.stringify(next[k as keyof Garment]),
    )
  )
    return listings
  return listings.map((l) =>
    l.garmentIds.includes(next.id) && ['approved', 'listed'].includes(l.status)
      ? { ...l, status: 'draft' }
      : l,
  )
}
