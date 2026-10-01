import { useEffect, useState } from 'react'
import { Shirt, ArrowRight, KeyRound, LoaderCircle } from 'lucide-react'
import type { SessionInfo } from '../lib/api/SessionInfo'
import { api, post } from '../lib/http'

export function AuthScreen({
  setupRequired,
  error,
  retry,
  onSignedIn,
}: {
  setupRequired: boolean
  error: string
  retry: () => void
  onSignedIn: (session: SessionInfo, recoveryCode?: string | null) => void
}) {
  const [mode, setMode] = useState<'login' | 'setup' | 'join' | 'recover'>(
    setupRequired ? 'setup' : 'login',
  )
  const [invite, setInvite] = useState('')
  const [household, setHousehold] = useState('Our family')
  const [email, setEmail] = useState('')
  const [name, setName] = useState('')
  const [password, setPassword] = useState('')
  const [setupToken, setSetupToken] = useState('')
  const [recoveryCode, setRecoveryCode] = useState('')
  const [samples] = useState(false)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  useEffect(() => {
    const token = new URLSearchParams(location.search).get('invite')
    if (!token) {
      setMode(setupRequired ? 'setup' : 'login')
      return
    }
    setInvite(token)
    setMode('join')
    void api<{ email: string; householdName: string }>(
      `/invitations/${encodeURIComponent(token)}`,
    )
      .then((invitation) => {
        setEmail(invitation.email)
        setHousehold(invitation.householdName)
      })
      .catch((error) => setMessage(error.message))
  }, [setupRequired])
  function login() {
    history.replaceState(null, '', location.pathname)
    setInvite('')
    setMode('login')
    setMessage('')
    setPassword('')
  }
  return (
    <main className="auth-page">
      <div className="auth-brand">
        <span>
          <Shirt size={30} />
        </span>
        Little Wardrobe<span className="heading-dot">.</span>
      </div>
      <section className="auth-card">
        <h1>
          {mode === 'setup'
            ? 'Set up Little Wardrobe'
            : mode === 'join'
              ? 'Join your family'
              : mode === 'recover'
                ? 'Back to your wardrobe.'
                : 'Welcome home.'}
        </h1>
        <p>
          {mode === 'setup'
            ? 'Create the first account, then invite your family.'
            : mode === 'join'
              ? `Join ${household} to manage clothes together.`
              : mode === 'recover'
                ? 'Use the recovery code you saved when creating your account.'
                : 'Every size, every season, every little adventure.'}
        </p>
        <form
          className="editor-form"
          onSubmit={async (event) => {
            event.preventDefault()
            setBusy(true)
            setMessage('')
            try {
              const body =
                mode === 'setup'
                  ? {
                      setupToken,
                      email,
                      name,
                      password,
                      householdName: household,
                      timeZone:
                        Intl.DateTimeFormat().resolvedOptions().timeZone,
                      samples,
                    }
                  : mode === 'join'
                    ? { token: invite, email, name, password }
                    : mode === 'recover'
                      ? {
                          email,
                          recoveryCode: recoveryCode.trim(),
                          newPassword: password,
                        }
                      : { email, password }
              const result = await post<{
                session: SessionInfo
                recoveryCode: string | null
              }>(`/auth/${mode === 'recover' ? 'recover' : mode}`, body)
              if (mode === 'join')
                history.replaceState(null, '', location.pathname)
              onSignedIn(result.session, result.recoveryCode)
            } catch (error) {
              setMessage(
                error instanceof Error ? error.message : 'Please try again.',
              )
            } finally {
              setBusy(false)
            }
          }}
        >
          {mode === 'setup' && (
            <label>
              Setup token
              <input
                aria-label="Setup token"
                value={setupToken}
                onChange={(event) => setSetupToken(event.target.value)}
                required
                autoComplete="off"
                placeholder="From your server’s setup-token file or secret"
              />
              <small>
                This protects first-use setup. Find it in the server data
                directory or your deployment secret.
              </small>
            </label>
          )}
          {(mode === 'setup' || mode === 'join') && (
            <label>
              Your name
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                required
                autoComplete="name"
                maxLength={80}
                placeholder="Your name"
              />
            </label>
          )}
          <label>
            Email
            <input
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              required
              autoComplete="username"
              readOnly={mode === 'join'}
              maxLength={254}
              placeholder="you@example.com"
            />
          </label>
          {mode === 'recover' && (
            <label>
              Recovery code
              <textarea
                value={recoveryCode}
                onChange={(event) => setRecoveryCode(event.target.value)}
                required
                autoComplete="off"
                rows={2}
              />
            </label>
          )}
          <label>
            {mode === 'recover' ? 'New password' : 'Password'}
            <input
              type="password"
              aria-label={mode === 'recover' ? 'New password' : 'Password'}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              required
              minLength={mode === 'login' ? undefined : 12}
              maxLength={256}
              autoComplete={
                mode === 'login' ? 'current-password' : 'new-password'
              }
            />
            {mode !== 'login' && (
              <small>
                At least 12 characters. A memorable passphrase works well.
              </small>
            )}
          </label>
          {mode === 'setup' && (
            <>
              <label>
                Household name
                <input
                  value={household}
                  onChange={(event) => setHousehold(event.target.value)}
                  required
                  maxLength={80}
                />
              </label>
            </>
          )}
          {(message || error) && (
            <div className="form-error" role="alert">
              {message || error}
              {error && (
                <button type="button" className="text-button" onClick={retry}>
                  Try connecting again
                </button>
              )}
            </div>
          )}
          <button className="primary-button" type="submit" disabled={busy}>
            {busy ? (
              <LoaderCircle size={18} className="spin" />
            ) : (
              <ArrowRight size={18} />
            )}
            {busy
              ? 'Opening your basket…'
              : mode === 'setup'
                ? 'Create our household'
                : mode === 'join'
                  ? 'Join our household'
                  : mode === 'recover'
                    ? 'Reset password and sign in'
                    : 'Sign in'}
          </button>
        </form>
        {mode === 'login' ? (
          <button
            className="text-button auth-alternate"
            onClick={() => setMode('recover')}
          >
            <KeyRound size={15} />
            Forgot your password?
          </button>
        ) : (
          mode !== 'setup' && (
            <button className="text-button auth-alternate" onClick={login}>
              Back to sign in
            </button>
          )
        )}
      </section>
      <p className="auth-footnote">Made for your household. Hosted by you.</p>
    </main>
  )
}
