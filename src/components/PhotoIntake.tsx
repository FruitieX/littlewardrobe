import { useCallback, useEffect, useRef, useState } from 'react'
import { Camera, LoaderCircle, X } from 'lucide-react'
import { api, HttpError, post, uploadImage } from '../lib/http'
import { useApp } from '../lib/state'
import type { PhotoJobView } from '../lib/api/PhotoJobView'
import type { PhotoDefaults } from '../lib/api/PhotoDefaults'
import type { GarmentPhoto } from '../lib/api/GarmentPhoto'
import { categories, statuses } from '../lib/wardrobe'
import { PhotoButton } from './PhotoButton'
import { Modal } from './ui'

export type QuickDefaults = Partial<PhotoDefaults>
type Request = { id: string; photos: GarmentPhoto[]; defaults: QuickDefaults }

export function PhotoIntake({
  onOpen,
  defaults,
  onJobsChange,
}: {
  onOpen: (id: string) => void
  defaults: QuickDefaults
  onJobsChange: (jobs: PhotoJobView[]) => void
}) {
  const { session, status, state } = useApp()
  const [jobs, setJobs] = useState<PhotoJobView[]>([])
  const [open, setOpen] = useState(false)
  const [photos, setPhotos] = useState<GarmentPhoto[]>([])
  const [selected, setSelected] = useState<QuickDefaults>({})
  const [busy, setBusy] = useState('')
  const [error, setError] = useState('')
  const [jobError, setJobError] = useState('')
  const [pending, setPending] = useState<Request | null>(null)
  const uploadRetry = useRef<File | null>(null)
  const requestId = useRef('')
  const refresh = useCallback(() => {
    if (status !== 'offline')
      void api<PhotoJobView[]>('/ai/photo-jobs')
        .then(setJobs)
        .catch(() => {})
  }, [status])
  useEffect(() => {
    onJobsChange(jobs)
  }, [jobs, onJobsChange])
  useEffect(() => {
    if (!session.aiEnabled) return
    refresh()
    const timer = window.setInterval(refresh, 5000)
    window.addEventListener('focus', refresh)
    return () => {
      window.clearInterval(timer)
      window.removeEventListener('focus', refresh)
    }
  }, [session.aiEnabled, refresh])
  useEffect(() => {
    if (session.aiEnabled) refresh()
  }, [state, session.aiEnabled, refresh])
  function start() {
    if (!photos.length && !pending && !uploadRetry.current) {
      setSelected({ ...defaults })
      requestId.current = crypto.randomUUID()
      setError('')
    }
    setOpen(true)
  }
  async function upload(file: File) {
    uploadRetry.current = file
    setBusy('Uploading photo…')
    setError('')
    try {
      const url = await uploadImage(file)
      setPhotos((previous) => [
        ...previous,
        {
          url,
          kind: ['front', 'back', 'tag', 'detail'][previous.length] ?? 'detail',
        },
      ])
      uploadRetry.current = null
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Please try again.')
    } finally {
      setBusy('')
    }
  }
  async function submit() {
    setBusy('Adding clothing…')
    setError('')
    const request = pending ?? {
      id: requestId.current,
      photos,
      defaults: selected,
    }
    setPending(request)
    try {
      const created = await post<{ id: string; garmentId: string }>(
        '/ai/photo-jobs',
        request,
      )
      setJobs((previous) => [
        {
          id: created.id,
          garmentId: created.garmentId,
          status: 'queued',
          error: null,
        },
        ...previous.filter((job) => job.id !== created.id),
      ])
      setPending(null)
      setPhotos([])
      setOpen(false)
      refresh()
    } catch (error) {
      // A definitive rejection can be corrected. An uncertain acceptance keeps
      // the same request intact for a duplicate-safe retry.
      if (error instanceof HttpError && error.status < 500) setPending(null)
      setError(error instanceof Error ? error.message : 'Please try again.')
    } finally {
      setBusy('')
    }
  }
  async function manage(id: string, action: 'retry' | 'cancel') {
    setJobError('')
    try {
      await post(`/ai/photo-jobs/${id}`, { action })
      refresh()
    } catch (error) {
      setJobError(error instanceof Error ? error.message : 'Please try again.')
    }
  }
  if (!session.aiEnabled) return null
  const locked = !!busy || !!pending
  const choose = (key: keyof PhotoDefaults, value: string) =>
    setSelected((previous) => ({ ...previous, [key]: value || null }))
  return (
    <section className="background-intake" aria-label="Quick add clothes">
      <div className="background-intake-start">
        <button
          type="button"
          className="secondary-button"
          disabled={status === 'offline'}
          onClick={start}
        >
          <Camera size={17} /> Quick add
        </button>
        <small>
          Add photos now; AI finishes the details in the background.
        </small>
      </div>
      {jobError && (
        <p className="form-error" role="alert">
          {jobError}
        </p>
      )}
      {jobs.map((job) => {
        const garment = state.garments?.find((g) => g.id === job.garmentId)
        if (!garment) return null
        return (
          <div className="photo-job" key={job.id}>
            <span role="status">
              {job.status === 'failed' ? (
                'Photo details failed'
              ) : (
                <>
                  <LoaderCircle className="spin" size={15} /> Reading photos…
                </>
              )}
            </span>
            <button type="button" onClick={() => onOpen(job.garmentId)}>
              {garment.name}
            </button>
            {job.error && <small>{job.error}</small>}
            {job.status === 'failed' && (
              <button
                type="button"
                disabled={status === 'offline'}
                onClick={() => void manage(job.id, 'retry')}
              >
                Retry AI
              </button>
            )}
            <button
              type="button"
              disabled={status === 'offline'}
              onClick={() => void manage(job.id, 'cancel')}
            >
              {job.status === 'failed' ? 'Dismiss' : 'Cancel AI'}
            </button>
          </div>
        )
      })}
      <Modal
        open={open}
        onOpenChange={(value) => {
          if (!busy) setOpen(value)
        }}
        title="Quick add"
        description="One piece, up to four photos. Add it now and review the AI details later."
        className="quick-add-modal"
      >
        <form
          className="editor-form"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}
        >
          <div className="photo-intake">
            <div className="photo-strip">
              {photos.map((photo, index) => (
                <div className="photo-thumb" key={photo.url}>
                  <img src={photo.url} alt={`${photo.kind} photo`} />
                  <select
                    aria-label={`Photo ${index + 1} type`}
                    disabled={locked}
                    value={photo.kind}
                    onChange={(event) =>
                      setPhotos((previous) =>
                        previous.map((value, i) =>
                          i === index
                            ? { ...value, kind: event.target.value }
                            : value,
                        ),
                      )
                    }
                  >
                    {['front', 'back', 'tag', 'detail'].map((kind) => (
                      <option key={kind}>{kind}</option>
                    ))}
                  </select>
                  <button
                    type="button"
                    aria-label={`Remove photo ${index + 1}`}
                    disabled={locked}
                    onClick={() =>
                      setPhotos((previous) =>
                        previous.filter((_, i) => i !== index),
                      )
                    }
                  >
                    <X size={13} />
                  </button>
                </div>
              ))}
            </div>
            <PhotoButton
              label="Add photo"
              disabled={
                locked ||
                status === 'offline' ||
                photos.length >= 4 ||
                !!uploadRetry.current
              }
              onFile={(file) => void upload(file)}
            />
            <small>
              Front, back, tag or detail. Keep photos of the same piece
              together. JPG, PNG or WebP, up to 8 MB each.
            </small>
            {uploadRetry.current && !busy && (
              <div className="photo-actions">
                <button
                  className="secondary-button"
                  type="button"
                  disabled={status === 'offline'}
                  onClick={() => void upload(uploadRetry.current!)}
                >
                  Retry photo upload
                </button>
                <button
                  className="secondary-button"
                  type="button"
                  onClick={() => {
                    uploadRetry.current = null
                    setError('')
                  }}
                >
                  Discard this photo
                </button>
              </div>
            )}
          </div>
          <div className="form-row">
            <label>
              Worn by
              <select
                aria-label="Quick add child"
                disabled={locked}
                value={selected.childId ?? ''}
                onChange={(event) => choose('childId', event.target.value)}
              >
                <option value="">Shared / unassigned</option>
                {state.children?.map((child) => (
                  <option key={child.id} value={child.id}>
                    {child.name}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Location
              <select
                aria-label="Quick add location"
                disabled={locked}
                value={selected.locationId ?? ''}
                onChange={(event) => choose('locationId', event.target.value)}
              >
                <option value="">Not assigned</option>
                {state.locations?.map((location) => (
                  <option key={location.id} value={location.id}>
                    {location.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <details>
            <summary>Starting details</summary>
            <div className="details-fields">
              <label>
                Status
                <select
                  disabled={locked}
                  value={selected.status ?? 'stored'}
                  onChange={(event) => choose('status', event.target.value)}
                >
                  {Object.entries(statuses).map(([value, name]) => (
                    <option key={value} value={value}>
                      {name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Category
                <select
                  disabled={locked}
                  value={selected.category ?? ''}
                  onChange={(event) => choose('category', event.target.value)}
                >
                  <option value="">Let AI suggest</option>
                  {Object.entries(categories).map(([value, name]) => (
                    <option key={value} value={value}>
                      {name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="form-row">
                <label>
                  Size on tag
                  <input
                    disabled={locked}
                    maxLength={200}
                    value={selected.size ?? ''}
                    onChange={(event) => {
                      choose('size', event.target.value)
                      if (!selected.sizeSystem) choose('sizeSystem', 'other')
                    }}
                  />
                </label>
                <label>
                  Size system
                  <select
                    disabled={locked}
                    value={selected.sizeSystem ?? ''}
                    onChange={(event) =>
                      choose('sizeSystem', event.target.value)
                    }
                  >
                    <option value="">Let AI suggest</option>
                    {Object.entries({
                      cm: 'Height (cm)',
                      age: 'Age on tag',
                      'eu-shoe': 'EU shoe size',
                      alpha: 'XS / S / M…',
                      other: 'Other / unknown',
                    }).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <label>
                Season
                <select
                  disabled={locked}
                  value={selected.season ?? ''}
                  onChange={(event) => choose('season', event.target.value)}
                >
                  <option value="">Let AI suggest</option>
                  {['all', 'spring', 'summer', 'autumn', 'winter'].map(
                    (value) => (
                      <option key={value}>{value}</option>
                    ),
                  )}
                </select>
              </label>
            </div>
          </details>
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          {busy && (
            <p className="busy-note" role="status">
              <LoaderCircle className="spin" size={15} />
              {busy}
            </p>
          )}
          <div className="editor-footer">
            <button
              className="secondary-button"
              type="button"
              disabled={!!busy}
              onClick={() => setOpen(false)}
            >
              Cancel
            </button>
            <button
              className="primary-button"
              disabled={
                !!busy ||
                status === 'offline' ||
                !photos.length ||
                !!uploadRetry.current
              }
            >
              Add to wardrobe
            </button>
          </div>
        </form>
      </Modal>
    </section>
  )
}
