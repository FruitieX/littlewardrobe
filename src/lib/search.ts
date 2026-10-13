export function normalizeSearch(text: string) {
  return text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
}

/** Each word may match a different clothing field. */
export function matchesSearch(
  fields: (string | undefined | null)[],
  query: string,
) {
  const words = normalizeSearch(query).split(/\s+/).filter(Boolean)
  const haystack = fields
    .filter(Boolean)
    .map((field) => normalizeSearch(field!))
  return words.every((word) => haystack.some((field) => field.includes(word)))
}
