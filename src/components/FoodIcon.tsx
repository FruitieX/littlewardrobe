// Bundled Twemoji SVG illustrations give all browsers the same food artwork.
export function FoodIcon({
  value,
  className = '',
  image,
}: {
  value: string
  className?: string
  image?: string
}) {
  const code = [...value]
    .map((character) => character.codePointAt(0)!.toString(16))
    .filter((part) => part !== 'fe0f')
    .join('-')
  return (
    <img
      src={image || `/images/food/${code}.svg`}
      className={`food-icon ${className}`}
      alt=""
      aria-hidden="true"
      onError={(event) => {
        const fallback = `/images/food/${code}.svg`
        if (event.currentTarget.getAttribute('src') !== fallback)
          event.currentTarget.src = fallback
      }}
    />
  )
}
