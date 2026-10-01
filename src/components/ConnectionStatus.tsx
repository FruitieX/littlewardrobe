import { useState } from 'react'
import {
  Check,
  CloudOff,
  LoaderCircle,
  AlertCircle,
  RefreshCw,
  X,
} from 'lucide-react'
import { useApp } from '../lib/state'
import { Modal } from './ui'

export function ConnectionStatus() {
  const { status, pending, retry } = useApp()
  const [open, setOpen] = useState(false)
  const conflicts = pending.filter((entry) => entry.failed)
  return (
    <>
      <button
        className={`connection-pill ${status}`}
        onClick={() => (conflicts.length ? setOpen(true) : retry())}
        title={
          status === 'live'
            ? 'Shared with your household'
            : status === 'offline'
              ? 'Offline changes are saved on this device and will sync when reconnected'
              : 'Synchronizing household changes'
        }
      >
        {conflicts.length ? (
          <AlertCircle size={13} />
        ) : status === 'live' ? (
          <Check size={13} />
        ) : status === 'offline' ? (
          <CloudOff size={13} />
        ) : (
          <LoaderCircle size={13} className="spin" />
        )}
        <span>
          {conflicts.length
            ? 'Review changes'
            : status === 'live'
              ? 'Shared'
              : status === 'offline'
                ? 'Offline'
                : 'Syncing'}
          {pending.length ? ` · ${pending.length}` : ''}
        </span>
      </button>
      <Modal
        open={open}
        onOpenChange={setOpen}
        title="A change needs your attention"
        description="Some changes could not be synced. Review them, then retry or discard them."
      >
        <PendingChanges />
      </Modal>
    </>
  )
}
export function PendingChanges() {
  const { pending, resolvePending, error, clearError, retry, status } = useApp()
  return (
    <div className="pending-changes">
      {error && (
        <div className="form-error" role="alert">
          {error}
          <button
            className="icon-button"
            aria-label="Dismiss message"
            onClick={clearError}
          >
            <X size={16} />
          </button>
        </div>
      )}
      {pending.map((entry) => (
        <div className="pending-change" key={entry.id}>
          <strong>
            {entry.operation.type === 'patch'
              ? entry.operation.label
              : entry.operation.type.replace(/([A-Z])/g, ' $1')}
          </strong>
          <p>{entry.failed || 'Saved on this device. Waiting to sync.'}</p>
          <div className="modal-actions">
            <button
              className="secondary-button"
              onClick={() =>
                entry.failed ? resolvePending(entry.id, false) : retry()
              }
            >
              <RefreshCw size={15} />
              Try syncing
            </button>
            <button
              className="text-button danger"
              onClick={() => resolvePending(entry.id, true)}
            >
              Discard this change
            </button>
          </div>
        </div>
      ))}
      {!pending.length && (
        <p>
          {status === 'offline'
            ? 'Your changes are saved. Reconnect to share them.'
            : 'Everything is synced with your household.'}
        </p>
      )}
    </div>
  )
}
