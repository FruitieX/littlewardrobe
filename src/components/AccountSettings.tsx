import { useState } from 'react'
import { Download, LogOut, Plus, Check, Users, Trash2 } from 'lucide-react'
import { useApp } from '../lib/state'
import { api, post } from '../lib/http'
import { Modal, SectionTitle } from './ui'
import { PendingChanges } from './ConnectionStatus'

export function AccountSettings() {
  const { session, signOut, refreshSession, pending, retry } = useApp()
  const [name, setName] = useState(session.user.name)
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [inviteEmail, setInviteEmail] = useState('')
  const [inviteUrl, setInviteUrl] = useState('')
  const [remove, setRemove] = useState<string | null>(null)
  const [error, setError] = useState('')
  const [saved, setSaved] = useState(false)
  const [busy, setBusy] = useState(false)
  const [cleanupMessage, setCleanupMessage] = useState('')
  const [recoveryCode, setRecoveryCode] = useState('')
  async function run(action: () => Promise<void>) {
    setBusy(true)
    setError('')
    try {
      await action()
    } catch (error) {
      setError(error instanceof Error ? error.message : 'Please try again.')
    } finally {
      setBusy(false)
    }
  }
  return (
    <>
      <section className="settings-card">
        <SectionTitle title="Your account" />
        <form
          className="editor-form"
          onSubmit={(event) => {
            event.preventDefault()
            void run(async () => {
              await api('/auth/profile', {
                method: 'PUT',
                body: JSON.stringify({
                  name,
                  currentPassword: newPassword ? currentPassword : null,
                  newPassword: newPassword || null,
                }),
              })
              await refreshSession()
              setCurrentPassword('')
              setNewPassword('')
              setSaved(true)
            })
          }}
        >
          <label>
            Your name
            <input
              required
              value={name}
              onChange={(event) => {
                setName(event.target.value)
                setSaved(false)
              }}
              maxLength={80}
              autoComplete="name"
            />
          </label>
          <p className="quiet-label">Signed in as {session.user.email}</p>
          <details className="settings-disclosure">
            <summary>Change password</summary>
            <label>
              Current password
              <input
                type="password"
                value={currentPassword}
                onChange={(event) => setCurrentPassword(event.target.value)}
                autoComplete="current-password"
              />
            </label>
            <label>
              New password
              <input
                type="password"
                value={newPassword}
                onChange={(event) => setNewPassword(event.target.value)}
                minLength={12}
                maxLength={256}
                autoComplete="new-password"
              />
              <small>
                At least 12 characters. Other devices will be signed out.
              </small>
            </label>
          </details>
          <details className="settings-disclosure">
            <summary>Replace recovery code</summary>
            <p className="quiet-label">
              This invalidates your old code. Save the replacement safely.
            </p>
            <label>
              Password to replace recovery code
              <input
                aria-label="Password to replace recovery code"
                type="password"
                autoComplete="current-password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
              />
            </label>
            <button
              type="button"
              className="secondary-button"
              disabled={busy || !currentPassword}
              onClick={() =>
                void run(async () => {
                  const result = await post<{ recoveryCode: string }>(
                    '/auth/recovery-code',
                    { currentPassword },
                  )
                  setRecoveryCode(result.recoveryCode)
                  setCurrentPassword('')
                })
              }
            >
              Create recovery code
            </button>
            {recoveryCode && (
              <code className="recovery-code">{recoveryCode}</code>
            )}
          </details>
          <button className="secondary-button" disabled={busy}>
            {saved ? <Check size={16} /> : null}
            {saved ? 'Account saved' : 'Save account'}
          </button>
        </form>
        <button
          className="text-button"
          onClick={() => void run(signOut)}
          disabled={busy}
        >
          <LogOut size={16} />
          Sign out
        </button>
      </section>
      <section className="settings-card">
        <SectionTitle title={session.householdName}>
          <Users size={18} />
        </SectionTitle>
        <div className="member-list">
          {session.members.map((member) => (
            <div key={member.id}>
              <span className="member-avatar">
                {member.name.slice(0, 1).toUpperCase()}
              </span>
              <span>
                <strong>{member.name}</strong>
                <small>
                  {member.email} · {member.role}
                </small>
              </span>
              {session.user.role === 'owner' &&
                member.id !== session.user.id && (
                  <button
                    className="icon-button danger"
                    aria-label={`Remove ${member.name}`}
                    onClick={() => setRemove(member.id)}
                  >
                    <Trash2 size={16} />
                  </button>
                )}
            </div>
          ))}
        </div>
        {session.user.role === 'owner' && (
          <form
            className="editor-form"
            onSubmit={(event) => {
              event.preventDefault()
              void run(async () => {
                const result = await post<{ url: string }>('/invitations', {
                  email: inviteEmail,
                })
                setInviteUrl(result.url)
              })
            }}
          >
            <label>
              Invite a family member
              <input
                type="email"
                value={inviteEmail}
                onChange={(event) => {
                  setInviteEmail(event.target.value)
                  setInviteUrl('')
                }}
                required
                placeholder="Their email address"
                autoComplete="off"
              />
            </label>
            <button className="secondary-button" disabled={busy}>
              <Plus size={15} />
              Create invitation
            </button>
            {inviteUrl && (
              <div className="invitation-result">
                <p>
                  Share this private invitation link. It expires in seven days
                  and can be used once.
                </p>
                <input
                  aria-label="Invitation link"
                  value={inviteUrl}
                  readOnly
                  onFocus={(event) => event.target.select()}
                />
                <button
                  type="button"
                  className="text-button"
                  onClick={() =>
                    void navigator.clipboard
                      .writeText(inviteUrl)
                      .catch(() =>
                        setError(
                          'Select the invitation link and copy it manually.',
                        ),
                      )
                  }
                >
                  Copy link
                </button>
              </div>
            )}
          </form>
        )}
      </section>
      <section className="settings-card">
        <SectionTitle title="Sync & backups" />
        <PendingChanges />
        {session.user.role === 'owner' && (
          <>
            <p className="settings-description">
              Download clothes, children, locations, sale drafts, history, and
              attached images. Account credentials and AI secrets stay on the
              server.
            </p>
            <div className="backup-actions">
              <a className="secondary-button" href="/api/backup" download>
                <Download size={16} />
                Download household backup
              </a>
              <button
                className="secondary-button"
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    const result = await post<{ removed: number }>(
                      '/images/cleanup',
                      {},
                    )
                    setCleanupMessage(
                      `${result.removed} unused photos removed. Recent uploads and photos needed for undo are kept.`,
                    )
                  })
                }
              >
                Free unused photo storage
              </button>
            </div>
            {cleanupMessage && (
              <p className="success-message" role="status">
                {cleanupMessage}
              </p>
            )}
            <label className="backup-restore secondary-button">
              Restore household backup
              <input
                aria-label="Restore household backup"
                type="file"
                accept="application/json,.json"
                disabled={busy || pending.length > 0}
                onChange={(event) => {
                  const file = event.target.files?.[0]
                  event.target.value = ''
                  if (!file) return
                  if (
                    !confirm(
                      'Replace your household inventory and history with this backup? Download your current backup first. Accounts and AI settings stay as they are.',
                    )
                  )
                    return
                  void run(async () => {
                    if (file.size > 90 * 1024 * 1024)
                      throw new Error(
                        'Use a full database restore for backups larger than 90 MB.',
                      )
                    const snapshot = await api<{ revision: number }>('/state')
                    await post('/restore', {
                      backup: JSON.parse(await file.text()),
                      expectedRevision: snapshot.revision,
                    })
                    retry()
                  })
                }}
              />
            </label>
          </>
        )}
        {pending.length > 0 && (
          <p className="quiet-label">
            Backups contain the server’s saved version. Sync outstanding changes
            first.
          </p>
        )}
      </section>
      {error && (
        <div className="form-error" role="alert">
          {error}
        </div>
      )}
      <Modal
        open={!!remove}
        onOpenChange={(value) => {
          if (!value) setRemove(null)
        }}
        title="Remove this member?"
        description="They will lose access to this household and their sessions will be signed out. Household clothes and history will remain."
      >
        <div className="modal-actions">
          <button className="secondary-button" onClick={() => setRemove(null)}>
            Keep member
          </button>
          <button
            className="primary-button"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                await api(`/members/${remove}`, { method: 'DELETE' })
                await refreshSession()
                setRemove(null)
              })
            }
          >
            Remove member
          </button>
        </div>
      </Modal>
    </>
  )
}
