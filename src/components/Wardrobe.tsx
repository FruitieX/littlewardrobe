import { useMemo, useState } from 'react'
import {
  Shirt,
  Plus,
  Search,
  SlidersHorizontal,
  Sparkles,
  ArrowRight,
  Box,
  Camera,
  Check,
  X,
  Heart,
  Settings,
  Tag,
  LayoutGrid,
  CircleHelp,
  Download,
  Copy,
  ExternalLink,
  LoaderCircle,
  RotateCcw,
  Users,
  ArrowUpRight,
} from 'lucide-react'
import { useApp } from '../lib/state'
import { api, post, uploadImage } from '../lib/http'
import { Modal, SectionTitle } from './ui'
import { ConnectionStatus } from './ConnectionStatus'
import { AccountSettings } from './AccountSettings'
import { AISettings } from './AISettings'
import {
  Garment,
  SaleListing,
  Child,
  age,
  euro,
  categories,
  conditions,
  statuses,
  fits,
  blankGarment,
  manualListing,
  recordChanges,
} from '../lib/wardrobe'
import type { GarmentPhoto } from '../lib/api/GarmentPhoto'
import type { StorageLocation } from '../lib/api/StorageLocation'

const Icon = ({ type, size = 36 }: { type: string; size?: number }) =>
  type === 'shoes' ? (
    <span className="shoe-symbol" style={{ fontSize: size }}>
      ⌁
    </span>
  ) : (
    <Shirt size={size} strokeWidth={1.3} />
  )
function Pill({
  children,
  tone = 'sage',
}: {
  children: React.ReactNode
  tone?: string
}) {
  return <span className={`wardrobe-pill ${tone}`}>{children}</span>
}
function ErrorMessage({ error }: { error: string }) {
  return error ? (
    <p className="form-error" role="alert">
      {error}
    </p>
  ) : null
}

export function Wardrobe() {
  const { state, ready, status, session, pending, error, clearError, retry } =
    useApp()
  const [tab, setTab] = useState<'wardrobe' | 'sell' | 'family' | 'settings'>(
    'wardrobe',
  )
  const [child, setChild] = useState('all'),
    [filter, setFilter] = useState('owned'),
    [query, setQuery] = useState(''),
    [category, setCategory] = useState('all'),
    [size, setSize] = useState(''),
    [season, setSeason] = useState('all'),
    [location, setLocation] = useState('all')
  const [filters, setFilters] = useState(false),
    [editing, setEditing] = useState<Garment | null>(null)
  const garments = state.garments ?? [],
    children = state.children ?? [],
    locations = state.locations ?? [],
    listings = state.saleListings ?? []
  const owned = garments.filter((g) => !['sold', 'donated'].includes(g.status))
  const shown = useMemo(
    () =>
      garments.filter(
        (g) =>
          (child === 'all' || g.childId === child) &&
          (filter === 'owned'
            ? !['sold', 'donated'].includes(g.status)
            : g.status === filter) &&
          (category === 'all' || category === g.category) &&
          (!size || g.size.toLowerCase().includes(size.toLowerCase())) &&
          (season === 'all' || g.season === season || g.season === 'all') &&
          (location === 'all' || g.locationId === location) &&
          [g.name, g.brand, g.size, g.color, g.notes]
            .join(' ')
            .toLowerCase()
            .includes(query.toLowerCase()),
      ),
    [garments, child, filter, category, size, season, location, query],
  )
  return (
    <div className="wardrobe-app" data-app-ready={ready}>
      <header className="wardrobe-header">
        <a
          className="wardrobe-brand"
          href="/"
          aria-label="Little Wardrobe home"
        >
          <span>
            <Shirt size={22} />
          </span>
          little wardrobe<span className="tiny-dot">.</span>
        </a>
        <nav aria-label="Main navigation">
          {(
            [
              { id: 'wardrobe', label: 'Wardrobe', icon: LayoutGrid },
              { id: 'sell', label: 'Sell', icon: Tag },
              { id: 'family', label: 'Family', icon: Users },
              { id: 'settings', label: 'Settings', icon: Settings },
            ] as const
          ).map((t) => (
            <button
              key={t.id}
              className={tab === t.id ? 'selected' : ''}
              onClick={() => setTab(t.id)}
            >
              <t.icon size={18} />
              <span>{t.label}</span>
              {t.id === 'sell' && garments.some((g) => g.status === 'sell') && (
                <i />
              )}
            </button>
          ))}
        </nav>
        <span className="household-label">
          <ConnectionStatus />
          {session.householdName}
          <span className={`sync-dot ${status}`} title={status} />
        </span>
      </header>
      <main className="wardrobe-main">
        {(status === 'offline' || pending.length > 0) && (
          <div className="sync-banner" role="status">
            {status === 'offline'
              ? 'Offline · saved changes will sync when you reconnect.'
              : `${pending.length} change${pending.length === 1 ? '' : 's'} syncing…`}
            <ConnectionStatus />
          </div>
        )}
        {error && (
          <div className="sync-banner error" role="alert">
            {error}
            <button onClick={clearError}>Dismiss</button>
          </div>
        )}
        {!ready ? (
          <p role="status">Opening your wardrobe…</p>
        ) : tab === 'wardrobe' ? (
          <>
            <section className="wardrobe-intro">
              <div>
                <div className="wardrobe-eyebrow">
                  SMALL CLOTHES. BIG ADVENTURES.
                </div>
                <h1>
                  A little room
                  <br />
                  to grow<span>.</span>
                </h1>
                <p>
                  What fits today. What’s tucked away.
                  <br className="desktop-only" /> What’s ready for its next
                  adventure.
                </p>
                <button
                  className="primary-button"
                  onClick={() => setEditing(blankGarment())}
                >
                  <Plus size={19} /> Add clothes
                </button>
              </div>
              <div className="intro-art" aria-hidden="true">
                <div className="art-hanger" />
                <div className="art-sweater">
                  <Shirt strokeWidth={0.9} />
                </div>
                <div className="art-card">
                  <Sparkles size={15} />
                  <span>A photo is a great start</span>
                </div>
                <span className="art-spark a">✧</span>
                <span className="art-spark b">✳</span>
                <span className="art-size">room to grow</span>
              </div>
            </section>
            <div className="wardrobe-stats">
              {[
                { label: 'In your wardrobe', n: owned.length, icon: Shirt },
                {
                  label: 'In rotation',
                  n: garments.filter((g) => g.status === 'active').length,
                  icon: RotateCcw,
                },
                {
                  label: 'Saved for later',
                  n: garments.filter((g) => g.status === 'stored').length,
                  icon: Box,
                },
                {
                  label: 'Ready to sell',
                  n: garments.filter((g) => g.status === 'sell').length,
                  icon: Tag,
                },
              ].map((s) => (
                <button
                  key={s.label}
                  onClick={() =>
                    setFilter(
                      s.label === 'In your wardrobe'
                        ? 'owned'
                        : s.label === 'In rotation'
                          ? 'active'
                          : s.label === 'Saved for later'
                            ? 'stored'
                            : 'sell',
                    )
                  }
                >
                  <span className="stat-icon">
                    <s.icon size={20} />
                  </span>
                  <div>
                    <strong>{s.n}</strong>
                    <span>{s.label}</span>
                  </div>
                </button>
              ))}
            </div>
            <section className="inventory-section">
              <div className="inventory-heading">
                <h2>
                  The wardrobe <span>{shown.length}</span>
                </h2>
                <div className="children-tabs">
                  <button
                    className={child === 'all' ? 'active' : ''}
                    onClick={() => setChild('all')}
                  >
                    Everyone
                  </button>
                  {children.map((c) => (
                    <button
                      key={c.id}
                      className={child === c.id ? 'active' : ''}
                      onClick={() => setChild(c.id)}
                    >
                      <span className={`child-dot ${c.color}`} />
                      {c.name}
                    </button>
                  ))}
                </div>
              </div>
              <div className="inventory-tools">
                <label className="search-field">
                  <Search size={18} />
                  <input
                    aria-label="Search clothes"
                    placeholder="Find a piece, a brand, a size…"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                  />
                </label>
                <button
                  className={`secondary-button ${filters ? 'active' : ''}`}
                  onClick={() => setFilters(!filters)}
                  aria-expanded={filters}
                >
                  <SlidersHorizontal size={17} /> Filters
                </button>
              </div>
              <div className="status-tabs">
                {[
                  'owned',
                  'active',
                  'stored',
                  'sell',
                  'reserved',
                  'keepsake',
                  'sold',
                  'donated',
                ].map((s) => (
                  <button
                    key={s}
                    className={filter === s ? 'active' : ''}
                    onClick={() => setFilter(s)}
                  >
                    {s === 'owned'
                      ? 'All clothes'
                      : statuses[s as keyof typeof statuses]}
                  </button>
                ))}
              </div>
              {filters && (
                <div className="filter-grid">
                  <label>
                    Category
                    <select
                      value={category}
                      onChange={(e) => setCategory(e.target.value)}
                    >
                      <option value="all">All categories</option>
                      {Object.entries(categories).map(([v, n]) => (
                        <option key={v} value={v}>
                          {n}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    Size
                    <input
                      value={size}
                      onChange={(e) => setSize(e.target.value)}
                      placeholder="e.g. 98"
                    />
                  </label>
                  <label>
                    Season
                    <select
                      value={season}
                      onChange={(e) => setSeason(e.target.value)}
                    >
                      {['all', 'spring', 'summer', 'autumn', 'winter'].map(
                        (s) => (
                          <option key={s} value={s}>
                            {s === 'all' ? 'All seasons' : s}
                          </option>
                        ),
                      )}
                    </select>
                  </label>
                  <label>
                    Location
                    <select
                      value={location}
                      onChange={(e) => setLocation(e.target.value)}
                    >
                      <option value="all">Everywhere</option>
                      {locations.map((l) => (
                        <option key={l.id} value={l.id}>
                          {l.name}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
              )}
              {shown.length ? (
                <div className="garment-grid">
                  {shown.map((g) => (
                    <button
                      className="garment-card"
                      key={g.id}
                      onClick={() => setEditing(g)}
                    >
                      <div className={`garment-image ${g.category}`}>
                        <GarmentImage g={g} />
                        {g.size && (
                          <span className="size-label">
                            {g.size}
                            {g.sizeSystem === 'cm' ? ' cm' : ''}
                          </span>
                        )}
                        {g.status === 'keepsake' && (
                          <Heart className="keepsake-heart" size={19} />
                        )}
                      </div>
                      <div className="garment-card-body">
                        <div className="brand-line">
                          {g.brand ||
                            categories[g.category as keyof typeof categories]}
                        </div>
                        <h3>{g.name}</h3>
                        <div className="garment-card-meta">
                          <Pill
                            tone={
                              g.status === 'sell'
                                ? 'peach'
                                : g.status === 'stored'
                                  ? 'neutral'
                                  : 'sage'
                            }
                          >
                            {statuses[g.status as keyof typeof statuses]}
                          </Pill>
                          <span>
                            {children.find((c) => c.id === g.childId)?.name ||
                              'Shared'}
                          </span>
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="wardrobe-empty">
                  <span>
                    <Shirt size={34} strokeWidth={1.2} />
                  </span>
                  <h3>
                    {garments.length
                      ? 'No clothes match just yet'
                      : 'Start with one little piece'}
                  </h3>
                  <p>
                    {garments.length
                      ? 'Try a different filter or search.'
                      : 'Take a photo, add the tag if needed, and let AI help with the details.'}
                  </p>
                  <button
                    className="secondary-button"
                    onClick={() => setEditing(blankGarment())}
                  >
                    <Camera size={17} /> Add your first piece
                  </button>
                </div>
              )}
            </section>
            {children.length > 0 && (
              <FitSuggestions
                children={children}
                garments={garments}
                edit={setEditing}
              />
            )}
          </>
        ) : tab === 'sell' ? (
          <Sales garments={garments} listings={listings} />
        ) : tab === 'family' ? (
          <Family
            children={children}
            locations={locations}
            garments={garments}
            edit={setEditing}
          />
        ) : (
          <>
            <div className="page-intro">
              <div className="wardrobe-eyebrow">MAKE YOURSELF AT HOME</div>
              <h1>
                Family settings<span>.</span>
              </h1>
              <p>Your accounts, backups and optional AI connection.</p>
            </div>
            <div className="settings-stack">
              <AISettings />
              <AccountSettings />
            </div>
          </>
        )}
        <footer className="wardrobe-footer">
          <Shirt size={15} /> A little less sorting. A little more living.
          <span>Made for your family</span>
        </footer>
      </main>
      {editing && (
        <GarmentEditor
          key={editing.id}
          initial={editing}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  )
}
function GarmentImage({ g }: { g: Garment }) {
  const photo =
    g.photos.find((p) => p.kind === 'front') ??
    g.photos.find((p) => !['tag', 'enhanced'].includes(p.kind))
  return photo ? (
    <img src={photo.url} alt={g.name} loading="lazy" />
  ) : (
    <Icon type={g.category} size={75} />
  )
}
function FitSuggestions({
  children,
  garments,
  edit,
}: {
  children: Child[]
  garments: Garment[]
  edit: (g: Garment) => void
}) {
  const suggestions = children
    .flatMap((c) =>
      garments
        .filter(
          (g) =>
            g.childId === c.id &&
            g.status === 'active' &&
            fits(g, c) === 'small',
        )
        .map((g) => ({ c, g })),
    )
    .slice(0, 6)
  if (!suggestions.length) return null
  return (
    <section className="fit-suggestions">
      <div>
        <Sparkles size={20} />
        <h2>Worth a size check</h2>
      </div>
      <p>
        These labels are below the current size you set. Actual fit can vary by
        brand.
      </p>
      {suggestions.map(({ c, g }) => (
        <button key={g.id} onClick={() => edit(g)}>
          <span>
            {g.name} · {g.size}
          </span>
          <span>
            {c.name} now wears {c.clothingSize}
            <ArrowRight size={17} />
          </span>
        </button>
      ))}
    </section>
  )
}

function GarmentEditor({
  initial,
  onClose,
}: {
  initial: Garment
  onClose: () => void
}) {
  const { state, update, action, session, status } = useApp()
  const [g, setG] = useState(initial),
    [busy, setBusy] = useState(''),
    [error, setError] = useState(''),
    [evidence, setEvidence] = useState<Record<string, string>>({}),
    [uncertain, setUncertain] = useState<string[]>([]),
    [needsTag, setNeedsTag] = useState(false)
  const children = state.children ?? [],
    locations = state.locations ?? [],
    exists = (state.garments ?? []).some((v) => v.id === g.id)
  const set = <K extends keyof Garment>(key: K, value: Garment[K]) =>
    setG((prev) => ({ ...prev, [key]: value }))
  async function run(label: string, fn: () => Promise<void>) {
    setBusy(label)
    setError('')
    try {
      await fn()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Please try again.')
    } finally {
      setBusy('')
    }
  }
  function save() {
    action({
      type: 'patch',
      label: exists ? 'Updated clothing' : 'Added clothing',
      changes: recordChanges('garments', exists ? initial : null, g),
    })
    onClose()
  }
  const text = (
    key: keyof Garment,
    label: string,
    placeholder = '',
    required = false,
  ) => (
    <label key={key}>
      {label}
      {uncertain.includes(key) && (
        <span className="uncertain"> Check this</span>
      )}
      <input
        value={String(g[key] ?? '')}
        onChange={(e) => set(key, e.target.value as never)}
        placeholder={placeholder}
        required={required}
        maxLength={200}
      />
      {evidence[key] && <small>{evidence[key]}</small>}
    </label>
  )
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose()
      }}
      title={exists ? 'A closer look' : 'Add a little piece'}
      description="Start with a photo. Review the details, then save it to your family wardrobe."
      className="garment-editor"
    >
      <form
        className="editor-form"
        onSubmit={(e) => {
          e.preventDefault()
          save()
        }}
      >
        <div className="photo-intake">
          <div className="photo-strip">
            {g.photos.map((p, i) => (
              <div className="photo-thumb" key={p.url}>
                <img src={p.url} alt={`${p.kind} photo`} />
                <select
                  aria-label={`Photo ${i + 1} type`}
                  disabled={p.kind === 'enhanced'}
                  value={p.kind}
                  onChange={(e) =>
                    set(
                      'photos',
                      g.photos.map((v, n) =>
                        n === i ? { ...v, kind: e.target.value } : v,
                      ),
                    )
                  }
                >
                  {(p.kind === 'enhanced'
                    ? ['enhanced']
                    : ['front', 'back', 'tag', 'detail']
                  ).map((k) => (
                    <option key={k}>{k}</option>
                  ))}
                </select>
                <button
                  type="button"
                  aria-label={`Remove photo ${i + 1}`}
                  onClick={() =>
                    set(
                      'photos',
                      g.photos.filter((_, n) => i !== n),
                    )
                  }
                >
                  <X size={13} />
                </button>
              </div>
            ))}
          </div>
          <div className="photo-actions">
            {(['front', 'tag'] as const).map((kind) => (
              <label className="secondary-button" key={kind}>
                <Camera size={17} />
                {kind === 'tag' ? 'Add tag photo' : 'Add clothing photo'}
                <input
                  type="file"
                  accept="image/jpeg,image/png,image/webp"
                  capture="environment"
                  aria-label={
                    kind === 'tag'
                      ? 'Upload tag photo'
                      : 'Upload clothing photo'
                  }
                  disabled={
                    !!busy || status === 'offline' || g.photos.length >= 12
                  }
                  onChange={(e) => {
                    const file = e.target.files?.[0]
                    if (file)
                      void run('Uploading photo', async () => {
                        const url = await uploadImage(file)
                        setG((v) => ({
                          ...v,
                          photos: [...v.photos, { url, kind }],
                        }))
                      })
                    e.target.value = ''
                  }}
                />
              </label>
            ))}
          </div>
          <small>
            One item per entry. JPG, PNG or WebP, up to 8 MB. Add front, back
            and detail photos as needed.
          </small>
        </div>
        {session.aiEnabled ? (
          <button
            type="button"
            className="ai-button"
            disabled={!!busy || status === 'offline' || !g.photos.length}
            onClick={() =>
              void run('Reading your photos', async () => {
                const result = await post<{
                  garment: Garment
                  evidence: Record<string, string>
                  uncertainFields: string[]
                  needsTagPhoto: boolean
                }>('/ai/analyze', {
                  photos: g.photos
                    .filter((p) => p.kind !== 'enhanced')
                    .slice(0, 4),
                  note: g.notes,
                })
                setG((v) => ({
                  ...v,
                  ...Object.fromEntries(
                    [
                      'name',
                      'category',
                      'brand',
                      'size',
                      'sizeSystem',
                      'color',
                      'material',
                      'season',
                      'condition',
                      'defects',
                      'notes',
                    ].map((key) => [key, result.garment[key as keyof Garment]]),
                  ),
                }))
                setEvidence(result.evidence)
                setUncertain(result.uncertainFields)
                setNeedsTag(result.needsTagPhoto)
              })
            }
          >
            <Sparkles size={17} /> Suggest details from photos
          </button>
        ) : (
          <p className="soft-note">
            <Sparkles size={16} />
            Connect AI in Settings to suggest details from photos. You can add
            everything manually.
          </p>
        )}
        {needsTag && (
          <p className="review-note">
            <CircleHelp size={17} />
            Size wasn’t readable. Add a close-up of the tag or enter the printed
            size.
          </p>
        )}
        {Object.keys(evidence).length > 0 && (
          <p className="review-note">
            AI filled in a draft. Check the size, brand and condition against
            the actual garment.
          </p>
        )}
        {text('name', 'Name', 'e.g. Striped cotton shirt', true)}
        <div className="form-row">
          <label>
            Category
            <select
              value={g.category}
              onChange={(e) => set('category', e.target.value)}
            >
              {Object.entries(categories).map(([v, n]) => (
                <option key={v} value={v}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          {text('brand', 'Brand', 'Optional')}
        </div>
        <div className="form-row">
          {text('size', 'Size on tag', 'e.g. 98 / 104')}
          <label>
            Size system
            <select
              value={g.sizeSystem}
              onChange={(e) => set('sizeSystem', e.target.value)}
            >
              {Object.entries({
                cm: 'Height (cm)',
                age: 'Age on tag',
                'eu-shoe': 'EU shoe size',
                alpha: 'XS / S / M…',
                other: 'Other / unknown',
              }).map(([v, n]) => (
                <option value={v} key={v}>
                  {n}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="form-row">
          <label>
            Condition{' '}
            <span className="optional">confirm by checking the item</span>
            <select
              value={g.condition}
              onChange={(e) => set('condition', e.target.value)}
            >
              {Object.entries(conditions).map(([v, n]) => (
                <option key={v} value={v}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          <label>
            Season
            <select
              value={g.season}
              onChange={(e) => set('season', e.target.value)}
            >
              {['all', 'spring', 'summer', 'autumn', 'winter'].map((v) => (
                <option key={v} value={v}>
                  {v === 'all' ? 'All seasons' : v}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label>
          Wear, stains or repairs
          <textarea
            value={g.defects}
            onChange={(e) => set('defects', e.target.value)}
            placeholder="Anything a future buyer should know"
            maxLength={2000}
          />
        </label>
        <div className="form-row">
          <label>
            Where is it in its journey?
            <select
              value={g.status}
              onChange={(e) => set('status', e.target.value)}
            >
              {Object.entries(statuses)
                .filter(([v]) => v !== 'sold')
                .map(([v, n]) => (
                  <option key={v} value={v}>
                    {n}
                  </option>
                ))}
              {g.status === 'sold' && <option value="sold">Sold</option>}
            </select>
          </label>
          <label>
            Worn by
            <select
              value={g.childId}
              onChange={(e) => set('childId', e.target.value)}
            >
              <option value="">Shared / unassigned</option>
              {children.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="form-row">
          <label>
            Save for
            <select
              aria-label="Save for"
              value={g.nextChildId}
              onChange={(e) => set('nextChildId', e.target.value)}
            >
              <option value="">Nobody selected</option>
              {children.map((c) => (
                <option value={c.id} key={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Storage location
            <select
              value={g.locationId}
              onChange={(e) => set('locationId', e.target.value)}
            >
              <option value="">Not set</option>
              {locations.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name}
                </option>
              ))}
            </select>
          </label>
        </div>
        {g.nextChildId && (
          <button
            type="button"
            className="secondary-button"
            onClick={() =>
              setG((v) => ({
                ...v,
                childId: v.nextChildId,
                nextChildId: '',
                status: 'active',
              }))
            }
          >
            <ArrowRight size={16} /> Pass on to{' '}
            {children.find((c) => c.id === g.nextChildId)?.name}
          </button>
        )}
        <details>
          <summary>More details & purchase history</summary>
          <div className="details-fields">
            <div className="form-row">
              {text('color', 'Color')}
              {text('material', 'Material')}
            </div>
            <div className="form-row">
              <label>
                Purchase price (€)
                <input
                  type="number"
                  min="0"
                  max="100000"
                  step="0.01"
                  value={g.purchasePrice ?? ''}
                  onChange={(e) =>
                    set(
                      'purchasePrice',
                      e.target.value ? Number(e.target.value) : null,
                    )
                  }
                />
              </label>
              <label>
                Purchase date
                <input
                  type="date"
                  value={g.purchaseDate}
                  onChange={(e) => set('purchaseDate', e.target.value)}
                />
              </label>
            </div>
            {text('purchaseSource', 'Bought from')}
            <label>
              Private notes
              <textarea
                value={g.notes}
                onChange={(e) => set('notes', e.target.value)}
                maxLength={2000}
                placeholder="Fit notes, washing instructions, memories…"
              />
            </label>
          </div>
        </details>
        {exists && session.imageEnabled && (
          <details>
            <summary>Prepare an optional edited photo</summary>
            <p>
              Creates a background edit from the actual photo. Compare it
              carefully: fabric, color and wear must stay accurate. Vinted
              exports always use originals.
            </p>
            <button
              type="button"
              className="secondary-button"
              disabled={
                !!busy ||
                status === 'offline' ||
                !g.photos.some((p) => !['tag', 'enhanced'].includes(p.kind))
              }
              onClick={() =>
                void run('Preparing a background edit', async () => {
                  const original = g.photos.find(
                    (p) => !['tag', 'enhanced'].includes(p.kind),
                  )!
                  const result = await post<{ url: string }>('/ai/enhance', {
                    garmentId: g.id,
                    photoUrl: original.url,
                  })
                  setG((v) => ({
                    ...v,
                    photos: [
                      ...v.photos,
                      { url: result.url, kind: 'enhanced' },
                    ],
                  }))
                })
              }
            >
              <Sparkles size={16} /> Create background edit
            </button>
          </details>
        )}
        {busy && (
          <p role="status" className="busy-note">
            <LoaderCircle size={17} className="spin" />
            {busy}…
          </p>
        )}
        <ErrorMessage error={error} />
        <div className="editor-footer">
          {exists &&
            !(state.saleListings ?? []).some((l) =>
              l.garmentIds.includes(g.id),
            ) && (
              <button
                type="button"
                className="text-button danger"
                onClick={() => {
                  if (
                    confirm(
                      'Delete this clothing entry? Its photos remain in your backup until unused photos are cleaned up.',
                    )
                  ) {
                    update(
                      (s) => ({
                        ...s,
                        garments: (s.garments ?? []).filter(
                          (v) => v.id !== g.id,
                        ),
                      }),
                      'Deleted clothing',
                    )
                    onClose()
                  }
                }}
              >
                Delete piece
              </button>
            )}
          <button
            type="button"
            className="secondary-button"
            onClick={onClose}
            disabled={!!busy}
          >
            Cancel
          </button>
          <button className="primary-button" disabled={!!busy}>
            <Check size={17} />
            {exists ? 'Save changes' : 'Add to wardrobe'}
          </button>
        </div>
      </form>
    </Modal>
  )
}

function Family({
  children,
  locations,
  garments,
  edit,
}: {
  children: Child[]
  locations: StorageLocation[]
  garments: Garment[]
  edit: (g: Garment) => void
}) {
  const { update, action } = useApp()
  const [childBefore, setChildBefore] = useState<Child | null>(null)
  const [locationBefore, setLocationBefore] = useState<StorageLocation | null>(
    null,
  )
  const [child, setChild] = useState<Child | null>(null),
    [location, setLocation] = useState<StorageLocation | null>(null)
  return (
    <>
      <div className="page-intro">
        <div className="wardrobe-eyebrow">FOR EVERY GROWING PERSON</div>
        <h1>
          A family of sizes<span>.</span>
        </h1>
        <p>Keep track of what fits, who’s next, and where it all lives.</p>
      </div>
      <section>
        <SectionTitle title="Your children">
          <button
            className="secondary-button"
            onClick={() => {
              setChildBefore(null)
              setChild({
                id: crypto.randomUUID(),
                name: '',
                birthDate: '',
                clothingSize: '',
                shoeSize: '',
                color: 'sage',
              })
            }}
          >
            <Plus size={17} /> Add child
          </button>
        </SectionTitle>
        <div className="family-grid">
          {children.map((c) => (
            <button
              className={`child-card ${c.color}`}
              key={c.id}
              onClick={() => {
                setChildBefore(c)
                setChild(c)
              }}
            >
              <div className="child-initial">{c.name[0]}</div>
              <h3>{c.name}</h3>
              <p>{age(c.birthDate) || 'Age not set'}</p>
              <div>
                <Pill tone="white">
                  {c.clothingSize
                    ? `Clothes ${c.clothingSize}`
                    : 'Size not set'}
                </Pill>
                {c.shoeSize && <Pill tone="white">Shoes {c.shoeSize}</Pill>}
              </div>
              <span>
                {
                  garments.filter(
                    (g) => g.childId === c.id && g.status === 'active',
                  ).length
                }{' '}
                pieces in rotation
                <ArrowUpRight size={17} />
              </span>
            </button>
          ))}
        </div>
        {!children.length && (
          <p className="soft-note">
            Add your children to assign clothes and spot sizes worth checking.
          </p>
        )}
      </section>
      <section className="locations-section">
        <SectionTitle title="A place for everything">
          <button
            className="secondary-button"
            onClick={() => {
              setLocationBefore(null)
              setLocation({ id: crypto.randomUUID(), name: '', note: '' })
            }}
          >
            <Plus size={17} /> Add location
          </button>
        </SectionTitle>
        <div className="location-grid">
          {locations.map((l) => (
            <div className="location-card" key={l.id}>
              <button
                onClick={() => {
                  setLocationBefore(l)
                  setLocation(l)
                }}
              >
                <Box size={25} />
                <span>
                  <strong>{l.name}</strong>
                  <small>{l.note || 'Storage location'}</small>
                </span>
                <span>
                  {
                    garments.filter(
                      (g) =>
                        g.locationId === l.id &&
                        !['sold', 'donated'].includes(g.status),
                    ).length
                  }{' '}
                  pieces
                </span>
              </button>
              <div>
                {garments
                  .filter(
                    (g) =>
                      g.locationId === l.id &&
                      !['sold', 'donated'].includes(g.status),
                  )
                  .slice(0, 4)
                  .map((g) => (
                    <button onClick={() => edit(g)} key={g.id}>
                      {g.name}
                      <ArrowRight size={14} />
                    </button>
                  ))}
              </div>
            </div>
          ))}
        </div>
      </section>
      {child && (
        <Modal
          open
          onOpenChange={(open) => {
            if (!open) setChild(null)
          }}
          title={
            children.some((c) => c.id === child.id)
              ? `About ${child.name}`
              : 'Meet a little person'
          }
          description="Sizes are editable. Age is calculated from the optional birthday."
        >
          <form
            className="editor-form"
            onSubmit={(e) => {
              e.preventDefault()
              action({
                type: 'patch',
                label: 'Updated children',
                changes: recordChanges('children', childBefore, child),
              })
              setChild(null)
            }}
          >
            <label>
              Name
              <input
                aria-label="Child name"
                required
                maxLength={80}
                value={child.name}
                onChange={(e) => setChild({ ...child, name: e.target.value })}
              />
            </label>
            <label>
              Birthday <span className="optional">optional</span>
              <input
                type="date"
                max={new Date().toISOString().slice(0, 10)}
                value={child.birthDate}
                onChange={(e) =>
                  setChild({ ...child, birthDate: e.target.value })
                }
              />
            </label>
            <div className="form-row">
              <label>
                Current clothing size (cm)
                <input
                  value={child.clothingSize}
                  onChange={(e) =>
                    setChild({ ...child, clothingSize: e.target.value })
                  }
                  placeholder="e.g. 104"
                  maxLength={80}
                />
              </label>
              <label>
                Current EU shoe size
                <input
                  value={child.shoeSize}
                  onChange={(e) =>
                    setChild({ ...child, shoeSize: e.target.value })
                  }
                  maxLength={80}
                />
              </label>
            </div>
            <label>
              Color
              <select
                aria-label="Child color"
                value={child.color}
                onChange={(e) => setChild({ ...child, color: e.target.value })}
              >
                {['sage', 'peach', 'blue', 'lilac'].map((v) => (
                  <option key={v}>{v}</option>
                ))}
              </select>
            </label>
            <div className="editor-footer">
              {children.some((c) => c.id === child.id) && (
                <button
                  type="button"
                  className="text-button danger"
                  onClick={() => {
                    if (
                      confirm(
                        'Remove this child? Their clothes will become unassigned.',
                      )
                    ) {
                      update(
                        (s) => ({
                          ...s,
                          children: children.filter((c) => c.id !== child.id),
                          garments: (s.garments ?? []).map((g) => ({
                            ...g,
                            childId: g.childId === child.id ? '' : g.childId,
                            nextChildId:
                              g.nextChildId === child.id ? '' : g.nextChildId,
                          })),
                        }),
                        'Removed child',
                      )
                      setChild(null)
                    }
                  }}
                >
                  Remove child
                </button>
              )}
              <button className="primary-button">Save child</button>
            </div>
          </form>
        </Modal>
      )}
      {location && (
        <Modal
          open
          onOpenChange={(open) => {
            if (!open) setLocation(null)
          }}
          title="Storage location"
          description="A drawer, box, bag or shelf. Give it a name you’ll recognize."
        >
          <form
            className="editor-form"
            onSubmit={(e) => {
              e.preventDefault()
              action({
                type: 'patch',
                label: 'Updated storage',
                changes: recordChanges('locations', locationBefore, location),
              })
              setLocation(null)
            }}
          >
            <label>
              Name
              <input
                aria-label="Location name"
                required
                maxLength={100}
                value={location.name}
                onChange={(e) =>
                  setLocation({ ...location, name: e.target.value })
                }
                placeholder="e.g. Attic · winter box"
              />
            </label>
            <label>
              Note
              <input
                maxLength={500}
                value={location.note}
                onChange={(e) =>
                  setLocation({ ...location, note: e.target.value })
                }
                placeholder="Blue lid, shelf 2"
              />
            </label>
            <div className="editor-footer">
              {locations.some((l) => l.id === location.id) && (
                <button
                  type="button"
                  className="text-button danger"
                  onClick={() => {
                    if (
                      confirm(
                        'Remove this location? The clothes will stay in your wardrobe.',
                      )
                    ) {
                      update(
                        (s) => ({
                          ...s,
                          locations: locations.filter(
                            (l) => l.id !== location.id,
                          ),
                          garments: (s.garments ?? []).map((g) => ({
                            ...g,
                            locationId:
                              g.locationId === location.id ? '' : g.locationId,
                          })),
                        }),
                        'Removed storage',
                      )
                      setLocation(null)
                    }
                  }}
                >
                  Remove location
                </button>
              )}
              <button className="primary-button">Save location</button>
            </div>
          </form>
        </Modal>
      )}
    </>
  )
}

function Sales({
  garments,
  listings,
}: {
  garments: Garment[]
  listings: SaleListing[]
}) {
  const { update, session, status } = useApp()
  const [selected, setSelected] = useState<string[]>([]),
    [platform, setPlatform] = useState('tori'),
    [editing, setEditing] = useState<SaleListing | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState('')
  const ready = garments.filter((g) => g.status === 'sell')
  async function draft(ai: boolean) {
    setBusy(true)
    setError('')
    try {
      const items = garments.filter((g) => selected.includes(g.id))
      const listing = ai
        ? (
            await post<{ listing: SaleListing }>('/ai/listing', {
              garmentIds: selected,
              platform,
            })
          ).listing
        : manualListing(items, platform)
      setEditing(listing)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Please try again.')
    } finally {
      setBusy(false)
    }
  }
  return (
    <>
      <div className="page-intro">
        <div className="wardrobe-eyebrow">OFF TO THEIR NEXT ADVENTURE</div>
        <h1>
          A new home awaits<span>.</span>
        </h1>
        <p>Turn outgrown pieces into thoughtful Finnish sale listings.</p>
      </div>
      <section className="sales-ready">
        <SectionTitle title="Ready to sell" count={ready.length} />
        {ready.length ? (
          <>
            <div className="sell-selection">
              {ready.map((g) => (
                <label
                  key={g.id}
                  className={selected.includes(g.id) ? 'selected' : ''}
                >
                  <input
                    type="checkbox"
                    checked={selected.includes(g.id)}
                    onChange={(e) =>
                      setSelected(
                        e.target.checked
                          ? [...selected, g.id]
                          : selected.filter((id) => id !== g.id),
                      )
                    }
                  />
                  <div className="sell-thumb">
                    <GarmentImage g={g} />
                  </div>
                  <div>
                    <strong>{g.name}</strong>
                    <span>
                      {g.brand} · {g.size || 'Size to check'}
                    </span>
                  </div>
                </label>
              ))}
            </div>
            <div className="sale-draft-actions">
              <label>
                Prepare for
                <select
                  aria-label="Listing platform"
                  value={platform}
                  onChange={(e) => setPlatform(e.target.value)}
                >
                  <option value="tori">Tori</option>
                  <option value="vinted">Vinted</option>
                </select>
              </label>
              <button
                className="secondary-button"
                disabled={!selected.length || busy}
                onClick={() => void draft(false)}
              >
                Create draft
              </button>
              {session.aiEnabled && (
                <button
                  className="primary-button"
                  disabled={!selected.length || busy || status === 'offline'}
                  onClick={() => void draft(true)}
                >
                  <Sparkles size={17} />
                  {busy
                    ? 'Writing…'
                    : `Suggest ${selected.length > 1 ? 'bundle' : 'listing'}`}
                </button>
              )}
            </div>
            <p className="soft-note">
              Select several pieces to make a bundle. Prices are reviewed
              estimates; publishing is a separate step.
            </p>
          </>
        ) : (
          <div className="wardrobe-empty compact">
            <Tag size={32} />
            <h3>Nothing waiting for its next home</h3>
            <p>Open a piece in the wardrobe and choose “Ready to sell”.</p>
          </div>
        )}
        <ErrorMessage error={error} />
      </section>
      <section className="listing-section">
        <SectionTitle title="Your sale desk" count={listings.length} />
        <div className="listing-grid">
          {[...listings]
            .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
            .map((l) => (
              <button
                className="listing-card"
                key={l.id}
                onClick={() => setEditing(l)}
              >
                <div>
                  <Pill tone={l.platform === 'tori' ? 'blue' : 'sage'}>
                    {l.platform === 'tori' ? 'Tori' : 'Vinted'}
                  </Pill>
                  <Pill tone="neutral">{l.status}</Pill>
                </div>
                <h3>{l.title}</h3>
                <p>
                  {l.garmentIds.length}{' '}
                  {l.garmentIds.length === 1 ? 'piece' : 'pieces'}
                </p>
                <strong>
                  {euro(
                    l.status === 'sold' && l.soldPrice !== null
                      ? l.soldPrice
                      : l.price,
                  )}
                </strong>
                <ArrowUpRight size={18} />
              </button>
            ))}
        </div>
      </section>
      {editing && (
        <ListingEditor
          key={editing.id}
          initial={editing}
          garments={garments}
          onClose={() => setEditing(null)}
        />
      )}
    </>
  )
}
function ListingEditor({
  initial,
  garments,
  onClose,
}: {
  initial: SaleListing
  garments: Garment[]
  onClose: () => void
}) {
  const { state, update, action, status } = useApp()
  const [reviewedItems] = useState(() =>
    garments.filter((g) => initial.garmentIds.includes(g.id)),
  )
  const [baseline, setBaseline] = useState<SaleListing | null>(
    (state.saleListings ?? []).find((v) => v.id === initial.id) ?? null,
  )
  const [l, setL] = useState(initial),
    [message, setMessage] = useState(''),
    [error, setError] = useState(''),
    [busy, setBusy] = useState(false)
  const items = garments.filter((g) => l.garmentIds.includes(g.id)),
    photos = items
      .flatMap((g) => g.photos)
      .filter((p) => l.platform !== 'vinted' || p.kind !== 'enhanced')
  const saved = (state.saleListings ?? []).some((v) => v.id === l.id),
    reviewed = ['approved', 'listed'].includes(l.status)
  function change<K extends keyof SaleListing>(key: K, value: SaleListing[K]) {
    setL((prev) => ({
      ...prev,
      [key]: value,
      status: ['approved', 'listed'].includes(prev.status)
        ? 'draft'
        : prev.status,
    }))
    setMessage('')
  }
  function save(next: SaleListing, close = false) {
    const changes =
      next.status === 'approved'
        ? [
            {
              collection: 'saleListings',
              key: next.id,
              field: null,
              before: baseline,
              after: next,
            },
            ...reviewedItems.map((g) => ({
              collection: 'garments',
              key: g.id,
              field: null,
              before: g,
              after: g,
            })),
          ]
        : recordChanges('saleListings', baseline, next)
    action({ type: 'patch', label: 'Updated sale draft', changes })
    setBaseline(next)
    setL(next)
    setMessage('Saved to your family sale desk.')
    if (close) onClose()
  }
  async function output(kind: 'copy' | 'download' | 'share') {
    setError('')
    setBusy(true)
    try {
      const current = await api<{ listing: SaleListing }>(
        `/sales/${l.id}/export`,
      )
      if (
        [
          'title',
          'description',
          'price',
          'photos',
          'platform',
          'garmentIds',
        ].some(
          (key) =>
            JSON.stringify(current.listing[key as keyof SaleListing]) !==
            JSON.stringify(l[key as keyof SaleListing]),
        )
      )
        throw new Error(
          'This draft changed on another device. Close and reopen it, then review the latest version.',
        )
      const text = `${l.title}\n\n${l.description}\n\nHinta: ${euro(l.price)}`
      if (kind === 'copy') await navigator.clipboard.writeText(text)
      else {
        const files = await Promise.all(
          l.photos.map(async (url, i) => {
            const r = await fetch(url, { credentials: 'same-origin' })
            if (!r.ok) throw new Error('A photo could not be downloaded.')
            const blob = await r.blob()
            return new File(
              [blob],
              `vaate-${i + 1}.${blob.type === 'image/webp' ? 'webp' : blob.type === 'image/png' ? 'png' : 'jpg'}`,
              { type: blob.type },
            )
          }),
        )
        files.unshift(new File([text], 'ilmoitus.txt', { type: 'text/plain' }))
        if (kind === 'share' && navigator.canShare?.({ files }))
          await navigator.share({ title: l.title, text: l.description, files })
        else
          for (const file of files) {
            const url = URL.createObjectURL(file)
            const a = document.createElement('a')
            a.href = url
            a.download = file.name
            a.click()
            setTimeout(() => URL.revokeObjectURL(url), 60000)
          }
      }
      setMessage(
        kind === 'copy'
          ? 'Finnish listing copied.'
          : kind === 'share'
            ? 'Listing package shared, or downloaded if file sharing is unavailable.'
            : 'Listing text and photos downloaded. Your browser may ask to allow multiple downloads.',
      )
    } catch (e) {
      if (!(e instanceof DOMException && e.name === 'AbortError'))
        setError(e instanceof Error ? e.message : 'Export failed.')
    } finally {
      setBusy(false)
    }
  }
  const canApprove =
    l.title.trim() &&
    l.description.trim() &&
    l.photos.length &&
    items.every((g) => g.size && g.condition !== 'unknown')
  return (
    <Modal
      open
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
      title={
        l.garmentIds.length > 1
          ? 'A bundle for the next family'
          : 'Prepare a sale listing'
      }
      description={`Finnish listing for ${l.platform === 'tori' ? 'Tori' : 'Vinted'}. Review the facts, photos and price before approving.`}
      className="listing-editor"
    >
      <form
        className="editor-form"
        onSubmit={(e) => {
          e.preventDefault()
          save(l, true)
        }}
      >
        <div className="listing-item-summary">
          {items.map((g) => (
            <span key={g.id}>
              {g.name} · {g.size || 'size needed'}
            </span>
          ))}
        </div>
        <label>
          Title
          <input
            aria-label="Listing title"
            value={l.title}
            onChange={(e) => change('title', e.target.value)}
            required
            maxLength={150}
          />
        </label>
        <label>
          Description
          <textarea
            aria-label="Listing description"
            className="listing-description"
            value={l.description}
            onChange={(e) => change('description', e.target.value)}
            required
            maxLength={6000}
          />
        </label>
        <label>
          Asking price (€)
          <input
            aria-label="Asking price"
            type="number"
            min="0"
            max="100000"
            step="0.01"
            value={l.price}
            onChange={(e) => change('price', Number(e.target.value))}
            required
          />
        </label>
        <p className="review-note">
          <CircleHelp size={17} />
          {l.priceReason}
        </p>
        <fieldset>
          <legend>Photos to include</legend>
          <div className="listing-photos">
            {photos.map((p) => (
              <label
                key={p.url}
                className={l.photos.includes(p.url) ? 'selected' : ''}
              >
                <img src={p.url} alt={`${p.kind} sale photo`} />
                <input
                  type="checkbox"
                  checked={l.photos.includes(p.url)}
                  disabled={!l.photos.includes(p.url) && l.photos.length >= 12}
                  onChange={(e) =>
                    change(
                      'photos',
                      e.target.checked
                        ? [...l.photos, p.url]
                        : l.photos.filter((url) => url !== p.url),
                    )
                  }
                />
                <span>{p.kind}</span>
              </label>
            ))}
          </div>
        </fieldset>
        <p className="soft-note">
          {l.platform === 'vinted'
            ? 'Vinted uses original photos only. Show the actual condition and any defects.'
            : 'Use accurate photos of the actual items. Compare edited images against the originals before choosing them.'}
        </p>
        {l.status === 'withdrawn' ? (
          <div className="review-note">
            One or more items sold elsewhere. Remove this marketplace listing to
            prevent double selling.
            {l.externalUrl && (
              <a href={l.externalUrl} target="_blank" rel="noreferrer">
                Open listing
              </a>
            )}
          </div>
        ) : l.status === 'sold' ? (
          <p className="success-note">
            Sold {l.soldAt}
            {l.soldPrice !== null && ` for ${euro(l.soldPrice)}`}. Remove any
            other live listings for these items.
          </p>
        ) : !reviewed ? (
          <div className="approval-box">
            <h3>Ready for your review</h3>
            <p>
              Confirm every size, the condition, visible wear, photo accuracy
              and your asking price.
            </p>
            {!canApprove && (
              <small>
                Add photos, sizes and confirmed conditions before approval.
              </small>
            )}
            <button
              type="button"
              className="primary-button"
              disabled={!canApprove || status === 'offline' || busy}
              onClick={() => save({ ...l, status: 'approved' })}
            >
              <Check size={17} /> Approve listing
            </button>
          </div>
        ) : (
          <div className="approval-box approved">
            <h3>
              <Check size={18} /> Reviewed and approved
            </h3>
            <p>
              Changing text, price, photos or clothing facts resets approval.
            </p>
            <div className="export-actions">
              <button
                type="button"
                className="secondary-button"
                disabled={busy || status === 'offline'}
                onClick={() => void output('copy')}
              >
                <Copy size={16} />
                Copy Finnish text
              </button>
              <button
                type="button"
                className="secondary-button"
                disabled={busy || status === 'offline'}
                onClick={() => void output('download')}
              >
                <Download size={16} />
                Download photos & text
              </button>
              {!!navigator.share && (
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy || status === 'offline'}
                  onClick={() => void output('share')}
                >
                  Share package
                </button>
              )}
              <a
                className="secondary-button"
                href={
                  l.platform === 'tori'
                    ? 'https://www.tori.fi/'
                    : 'https://www.vinted.fi/items/new'
                }
                target="_blank"
                rel="noreferrer"
              >
                <ExternalLink size={16} />
                Open {l.platform === 'tori' ? 'Tori' : 'Vinted'}
              </a>
            </div>
            <p className="soft-note">
              Copy or share the package, then finish posting in the marketplace.
              Approved drafts are not automatically published.
            </p>
          </div>
        )}
        {reviewed && l.status !== 'sold' && (
          <>
            <label>
              Published listing link
              <input
                type="url"
                placeholder={
                  l.platform === 'tori'
                    ? 'https://www.tori.fi/…'
                    : 'https://www.vinted.fi/…'
                }
                value={l.externalUrl}
                maxLength={1000}
                onChange={(e) => setL({ ...l, externalUrl: e.target.value })}
              />
            </label>
            <div className="export-actions">
              <button
                type="button"
                className="secondary-button"
                disabled={!l.externalUrl}
                onClick={() => save({ ...l, status: 'listed' })}
              >
                <ExternalLink size={16} />
                Mark as listed
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => {
                  const price = prompt(
                    'Actual sale price for this listing (€):',
                    String(l.price),
                  )
                  if (price === null) return
                  const amount = Number(price.replace(',', '.'))
                  if (
                    !Number.isFinite(amount) ||
                    amount < 0 ||
                    amount > 100000
                  ) {
                    setError('Enter a valid sale price.')
                    return
                  }
                  const today = new Date().toISOString().slice(0, 10)
                  const sold = {
                    ...l,
                    status: 'sold',
                    soldPrice: amount,
                    soldAt: today,
                  }
                  action({
                    type: 'patch',
                    label: 'Sold clothing',
                    changes: [
                      {
                        collection: 'saleListings',
                        key: l.id,
                        field: null,
                        before: baseline,
                        after: sold,
                      },
                      ...items.flatMap((g) =>
                        recordChanges('garments', g, { ...g, status: 'sold' }),
                      ),
                      ...(state.saleListings ?? [])
                        .filter(
                          (v) =>
                            v.id !== l.id &&
                            !['sold', 'withdrawn'].includes(v.status) &&
                            v.garmentIds.some((id) =>
                              l.garmentIds.includes(id),
                            ),
                        )
                        .flatMap((v) =>
                          recordChanges('saleListings', v, {
                            ...v,
                            status: 'withdrawn',
                          }),
                        ),
                    ],
                  })
                  setBaseline(sold)
                  setL(sold)
                  setMessage(
                    'Sale recorded. Remove any other live marketplace listings for these items.',
                  )
                }}
              >
                <Check size={16} />
                Record sale
              </button>
            </div>
          </>
        )}
        {message && (
          <p role="status" className="success-note">
            {message}
          </p>
        )}
        <ErrorMessage error={error} />
        <div className="editor-footer">
          {saved && l.status === 'draft' && (
            <button
              type="button"
              className="text-button danger"
              onClick={() => {
                if (
                  confirm(
                    'Delete this sale draft? Clothing entries stay in your wardrobe.',
                  )
                ) {
                  update(
                    (s) => ({
                      ...s,
                      saleListings: (s.saleListings ?? []).filter(
                        (v) => v.id !== l.id,
                      ),
                    }),
                    'Deleted sale draft',
                  )
                  onClose()
                }
              }}
            >
              Delete draft
            </button>
          )}
          <button type="button" className="secondary-button" onClick={onClose}>
            Close
          </button>
          <button className="primary-button" disabled={busy}>
            <Check size={16} />
            Save draft
          </button>
        </div>
      </form>
    </Modal>
  )
}
