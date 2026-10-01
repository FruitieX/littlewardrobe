import { useEffect, useState } from 'react'

// Like the current store, list presentation belongs to this account/device.
export function useShoppingLayout(userId: string) {
  const key = `littlewardrobe:group-shopping:${userId}`
  const [groupByCategory, setGroupByCategory] = useState(false)
  useEffect(() => {
    const load = () => {
      try {
        setGroupByCategory(localStorage.getItem(key) === 'true')
      } catch {
        setGroupByCategory(false)
      }
    }
    load()
    const changed = (event: StorageEvent) => {
      if (event.key === key || event.key === null) load()
    }
    window.addEventListener('storage', changed)
    return () => window.removeEventListener('storage', changed)
  }, [key])
  return {
    groupByCategory,
    setGroupByCategory: (enabled: boolean) => {
      setGroupByCategory(enabled)
      try {
        localStorage.setItem(key, String(enabled))
      } catch {
        // The preference still works for this visit if storage is unavailable.
      }
    },
  }
}
