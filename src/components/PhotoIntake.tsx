import { useCallback, useEffect, useState } from 'react'
import { LoaderCircle } from 'lucide-react'
import { api, post, uploadImage } from '../lib/http'
import { useApp } from '../lib/state'
import type { PhotoJobView } from '../lib/api/PhotoJobView'
import { PhotoButton } from './PhotoButton'

export function PhotoIntake({ onOpen }: { onOpen: (id: string) => void }) {
  const { session, status, state } = useApp()
  const [jobs, setJobs] = useState<PhotoJobView[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [pending, setPending] = useState<{ id: string; url: string } | null>(
    null,
  )
  const refresh = useCallback(() => {
    if (status !== 'offline')
      void api<PhotoJobView[]>('/ai/photo-jobs')
        .then(setJobs)
        .catch(() => {})
  }, [status])
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
  async function capture(file?: File) {
    setBusy(true)
    setError('')
    try {
      const request = file
        ? { id: crypto.randomUUID(), url: await uploadImage(file) }
        : pending
      if (!request) return
      setPending(request)
      await post('/ai/photo-jobs', {
        id: request.id,
        photos: [{ url: request.url, kind: 'front' }],
      })
      setPending(null)
      refresh()
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Please try again.')
    } finally {
      setBusy(false)
    }
  }
  async function manage(id: string, action: 'retry' | 'cancel') {
    setError('')
    try {
      await post(`/ai/photo-jobs/${id}`, { action })
      refresh()
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Please try again.')
    }
  }
  if (!session.aiEnabled) return null
  return (
    <section className="background-intake" aria-label="Add clothes from photos">
      <div className="background-intake-start">
        <PhotoButton
          label="Add from photo"
          disabled={busy || status === 'offline' || !!pending}
          onFile={(file) => void capture(file)}
        />
        <small>
          {busy
            ? 'Uploading and saving your photo…'
            : 'One piece per photo. AI finishes the details in the background; review them later.'}
        </small>
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      {pending && !busy && (
        <button
          className="secondary-button"
          type="button"
          disabled={status === 'offline'}
          onClick={() => void capture()}
        >
          Retry adding this photo
        </button>
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
                  <LoaderCircle className="spin" size={15} /> Reading photo…
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
    </section>
  )
}
