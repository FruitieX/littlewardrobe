import { useEffect, useRef, useState } from 'react'
import { Camera, ChevronDown, Image as ImageIcon } from 'lucide-react'

/** Explicit camera/gallery choice, constrained to the server's supported formats. */
export function PhotoButton({
  label,
  onFile,
  disabled = false,
}: {
  label: string
  onFile: (file: File) => void
  disabled?: boolean
}) {
  const [menu, setMenu] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const camera = useRef<HTMLInputElement>(null)
  const gallery = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (!menu) return
    const close = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setMenu(false)
    }
    document.addEventListener('pointerdown', close)
    return () => document.removeEventListener('pointerdown', close)
  }, [menu])
  return (
    <div
      className="photo-split"
      ref={root}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.stopPropagation()
          setMenu(false)
        }
      }}
    >
      <button
        type="button"
        className="secondary-button"
        disabled={disabled}
        onClick={() => camera.current?.click()}
      >
        <Camera size={17} /> {label}
      </button>
      <button
        type="button"
        className="secondary-button photo-more"
        aria-label={`${label} options`}
        aria-expanded={menu}
        disabled={disabled}
        onClick={() => setMenu(!menu)}
      >
        <ChevronDown size={15} />
      </button>
      {menu && (
        <div className="photo-menu">
          <button
            type="button"
            onClick={() => {
              setMenu(false)
              camera.current?.click()
            }}
          >
            <Camera size={16} /> Take photo
          </button>
          <button
            type="button"
            onClick={() => {
              setMenu(false)
              gallery.current?.click()
            }}
          >
            <ImageIcon size={16} /> Choose from gallery
          </button>
        </div>
      )}
      {(['camera', 'gallery'] as const).map((kind) => (
        <input
          key={kind}
          ref={kind === 'camera' ? camera : gallery}
          type="file"
          hidden
          accept="image/jpeg,image/png,image/webp"
          capture={kind === 'camera' ? 'environment' : undefined}
          aria-label={kind === 'camera' ? label : `${label} from gallery`}
          disabled={disabled}
          onChange={(event) => {
            const file = event.target.files?.[0]
            event.target.value = ''
            if (file) onFile(file)
          }}
        />
      ))}
    </div>
  )
}
