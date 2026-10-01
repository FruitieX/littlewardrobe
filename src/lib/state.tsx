import { createContext, useContext, useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { initialState, recipes } from './data'
import type { AppState } from './data'
import type { SessionInfo } from './api/SessionInfo'
import type { Operation } from './api/Operation'
import type { Mutation } from './api/Mutation'
import { getShoppingItems } from './meal-shopping'
import { diffState, optimistic } from './sync'
import { api, HttpError, post } from './http'
import { AuthScreen } from '../components/AuthScreen'
import { Modal } from '../components/ui'

type Snapshot = { revision: number; state: AppState; eventId?: string }
type Pending = Mutation & { failed?: string; storeId?: string }
type SyncStatus = 'connecting' | 'live' | 'offline' | 'syncing'
type ContextValue = {
  state: AppState
  ready: boolean
  session: SessionInfo
  status: SyncStatus
  pending: Pending[]
  error: string
  clearError: () => void
  update: (fn: (state: AppState) => AppState, label?: string) => void
  action: (operation: Operation) => void
  retry: () => void
  resolvePending: (id: string, discard: boolean) => void
  signOut: () => Promise<void>
  refreshSession: () => Promise<void>
}
const Context = createContext<ContextValue | null>(null)
const cachedSession = 'littlewardrobe:session'
function read<T>(key: string): T | null {
  try {
    return JSON.parse(localStorage.getItem(key) ?? 'null') as T | null
  } catch {
    return null
  }
}
function write(key: string, value: unknown) {
  localStorage.setItem(key, JSON.stringify(value))
}
function queueKey(session: SessionInfo, tab: string) {
  return `littlewardrobe:queue:${session.householdId}:${session.user.id}:${tab}`
}
function cacheKey(session: SessionInfo) {
  return `littlewardrobe:state:${session.householdId}:${session.user.id}`
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AppState>(initialState)
  const [session, setSession] = useState<SessionInfo | null>(null)
  const [ready, setReady] = useState(false)
  const [booted, setBooted] = useState(false)
  const [setupRequired, setSetupRequired] = useState(false)
  const [status, setStatus] = useState<SyncStatus>('connecting')
  const [pending, setPending] = useState<Pending[]>([])
  const [error, setError] = useState('')
  const [recovery, setRecovery] = useState<string | null>(null)
  const server = useRef<Snapshot | null>(null)
  const current = useRef(state)
  const queue = useRef<Pending[]>([])
  const tab = useRef('')
  const shop = useRef('')
  const flushing = useRef(false)
  const activeSession = useRef<SessionInfo | null>(null)
  const refreshRef = useRef<() => Promise<void>>(async () => {})
  const flushRef = useRef<() => Promise<void>>(async () => {})

  async function boot() {
    setError('')
    setStatus('connecting')
    try {
      const me = await api<SessionInfo>('/auth/me')
      setSession(me)
      write(cachedSession, me)
    } catch (error) {
      if (error instanceof HttpError && error.status === 401) {
        localStorage.removeItem(cachedSession)
        try {
          setSetupRequired(
            (await api<{ setupRequired: boolean }>('/auth/status'))
              .setupRequired,
          )
        } catch {
          setError(
            'Cannot reach the server. Check your connection and try again.',
          )
        }
      } else {
        const cached = read<SessionInfo>(cachedSession)
        if (cached && read(cacheKey(cached))) {
          setSession(cached)
          setStatus('offline')
        } else
          setError(
            'Cannot reach the server. Check your connection and try again.',
          )
      }
    }
    setBooted(true)
  }
  useEffect(() => {
    void boot()
    if (import.meta.env.PROD && 'serviceWorker' in navigator) {
      void navigator.serviceWorker.register('/sw.js').catch(() => {})
    }
  }, [])

  function publish() {
    const base = server.current
    if (!base) return
    let next = base.state
    for (const mutation of queue.current)
      if (!mutation.failed) {
        try {
          next = optimistic(next, mutation.operation)
        } catch {
          /* Preserve the server snapshot if an offline target has vanished. */
        }
      }
    const selected = next.stores.some((store) => store.id === shop.current)
      ? shop.current
      : next.stores.some((store) => store.id === next.storeId)
        ? next.storeId
        : next.stores[0].id
    next = { ...next, storeId: selected }
    current.current = next
    setState(next)
    setPending([...queue.current])
    const me = activeSession.current
    if (me) {
      try {
        write(cacheKey(me), base)
        write(queueKey(me, tab.current), queue.current)
      } catch {
        setError(
          'This device cannot save offline changes. Keep this tab open until changes sync.',
        )
      }
    }
  }
  async function refreshSession() {
    const identity = activeSession.current?.user.id
    const me = await api<SessionInfo>('/auth/me')
    if (identity !== activeSession.current?.user.id) return
    activeSession.current = me
    setSession(me)
    write(cachedSession, me)
  }
  async function refresh() {
    if (!activeSession.current) return
    const identity = activeSession.current.user.id
    try {
      const snapshot = await api<Snapshot>('/state')
      if (identity !== activeSession.current?.user.id) return
      if (!server.current || snapshot.revision >= server.current.revision) {
        server.current = snapshot
        publish()
      }
      setReady(true)
      await flushRef.current()
    } catch (error) {
      if (identity !== activeSession.current?.user.id) return
      if (error instanceof HttpError && error.status === 401) {
        activeSession.current = null
        setSession(null)
        setReady(false)
        setError('Your session expired. Sign in to sync your saved changes.')
        localStorage.removeItem(cachedSession)
      } else {
        setStatus('offline')
        if (server.current) setReady(true)
      }
    }
  }
  refreshRef.current = refresh
  async function flush() {
    const me = activeSession.current
    if (flushing.current || !me || !server.current || !navigator.onLine) return
    flushing.current = true
    try {
      while (
        queue.current.length &&
        activeSession.current?.user.id === me.user.id
      ) {
        const entry = queue.current[0]
        if (entry.failed) {
          setStatus('live')
          break
        }
        setStatus('syncing')
        try {
          const { failed: _failed, ...mutation } = entry
          const snapshot = await post<Snapshot>('/operations', mutation)
          if (activeSession.current?.user.id !== me.user.id) return
          queue.current = queue.current.filter((item) => item.id !== entry.id)
          if (!server.current || snapshot.revision >= server.current.revision)
            server.current = snapshot
          publish()
        } catch (error) {
          if (activeSession.current?.user.id !== me.user.id) return
          if (error instanceof HttpError && error.status === 401) {
            setSession(null)
            setReady(false)
            setError('Sign in again to sync your saved changes.')
            break
          }
          if (
            error instanceof HttpError &&
            error.status >= 400 &&
            error.status < 500 &&
            error.status !== 429
          ) {
            entry.failed = error.message
            setError(error.message)
            try {
              const snapshot = await api<Snapshot>('/state')
              if (activeSession.current?.user.id !== me.user.id) return
              server.current = snapshot
            } catch {
              /* Keep the latest available snapshot. */
            }
            publish()
            setStatus('live')
          } else setStatus('offline')
          break
        }
      }
      if (!queue.current.length) setStatus('live')
    } finally {
      flushing.current = false
    }
  }
  flushRef.current = flush

  useEffect(() => {
    if (!session) {
      activeSession.current = null
      return
    }
    activeSession.current = session
    if (!tab.current) {
      // A fresh identity avoids cloned sessionStorage in duplicated browser tabs.
      tab.current = navigator.locks
        ? crypto.randomUUID()
        : (sessionStorage.getItem('littlewardrobe:tab') ?? crypto.randomUUID())
      sessionStorage.setItem('littlewardrobe:tab', tab.current)
    }
    shop.current =
      localStorage.getItem(`littlewardrobe:shop:${session.user.id}`) ?? ''
    server.current = read<Snapshot>(cacheKey(session))
    queue.current = read<Pending[]>(queueKey(session, tab.current)) ?? []
    if (server.current) {
      publish()
      setReady(true)
    }
    let disposed = false
    let releaseTab: (() => void) | undefined
    // A browser lock distinguishes an active tab from an abandoned persisted outbox.
    if (navigator.locks) {
      void navigator.locks.request(
        `littlewardrobe-tab:${tab.current}`,
        async () => {
          await new Promise<void>((resolve) => {
            releaseTab = resolve
            if (disposed) resolve()
          })
        },
      )
      const prefix = `littlewardrobe:queue:${session.householdId}:${session.user.id}:`
      void (async () => {
        for (const key of Object.keys(localStorage).filter(
          (key) =>
            key.startsWith(prefix) && key !== queueKey(session, tab.current),
        )) {
          const otherTab = key.slice(prefix.length)
          await navigator.locks.request(
            `littlewardrobe-tab:${otherTab}`,
            { ifAvailable: true },
            (lock) => {
              if (!lock || disposed) return
              const saved = read<Pending[]>(key) ?? []
              queue.current.push(
                ...saved.filter(
                  (entry) =>
                    !queue.current.some((queued) => queued.id === entry.id),
                ),
              )
              publish()
              localStorage.removeItem(key)
            },
          )
        }
        if (!disposed) void flushRef.current()
      })()
    }
    let events: EventSource | null = null
    function connect() {
      if (disposed || !navigator.onLine || events) return
      events = new EventSource('/api/events')
      events.onopen = () => {
        setStatus(queue.current.length ? 'syncing' : 'live')
        void refreshRef.current()
      }
      events.addEventListener('sync', () => {
        void refreshRef.current()
        void refreshSession().catch(() => {})
      })
      events.addEventListener('signedOut', () => {
        events?.close()
        events = null
        setSession(null)
        setReady(false)
        localStorage.removeItem(cachedSession)
      })
      events.onerror = () => setStatus('offline')
    }
    const online = () => {
      connect()
      void refreshRef.current()
    }
    const offline = () => setStatus('offline')
    const visible = () => {
      if (!document.hidden) void refreshRef.current()
    }
    void refresh()
    connect()
    const poll = window.setInterval(() => {
      if (!document.hidden && navigator.onLine) void refreshRef.current()
    }, 15000)
    window.addEventListener('online', online)
    window.addEventListener('offline', offline)
    document.addEventListener('visibilitychange', visible)
    return () => {
      disposed = true
      releaseTab?.()
      events?.close()
      clearInterval(poll)
      window.removeEventListener('online', online)
      window.removeEventListener('offline', offline)
      document.removeEventListener('visibilitychange', visible)
    }
  }, [session?.user.id])

  function action(operation: Operation) {
    if (!activeSession.current || !server.current) return
    queue.current.push({
      id: crypto.randomUUID(),
      operation,
      storeId: current.current.storeId,
    })
    publish()
    void flushRef.current()
  }
  function update(
    fn: (state: AppState) => AppState,
    label = 'Household update',
  ) {
    const before = current.current,
      after = fn(before)
    if (after.storeId !== before.storeId) {
      shop.current = after.storeId
      if (activeSession.current)
        localStorage.setItem(
          `littlewardrobe:shop:${activeSession.current.user.id}`,
          after.storeId,
        )
      current.current = { ...before, storeId: after.storeId }
      setState(current.current)
    }
    const changes = diffState(before, after)
    if (changes.length) action({ type: 'patch', changes, label })
  }
  async function signOut() {
    if (queue.current.length) {
      setError(
        'There are unsynced changes. Sync or discard them before signing out.',
      )
      return
    }
    await post('/auth/logout', {})
    if (session) {
      localStorage.removeItem(cacheKey(session))
      localStorage.removeItem(queueKey(session, tab.current))
    }
    localStorage.removeItem(cachedSession)
    server.current = null
    setSession(null)
    setReady(false)
    navigator.serviceWorker?.controller?.postMessage({
      type: 'CLEAR_PRIVATE_CACHE',
    })
  }
  function resolvePending(id: string, discard: boolean) {
    if (discard)
      queue.current = queue.current.filter((entry) => entry.id !== id)
    else {
      const entry = queue.current.find((entry) => entry.id === id)
      if (entry) {
        entry.failed = undefined
        entry.id = crypto.randomUUID()
      }
    }
    setError('')
    publish()
    void flushRef.current()
  }

  if (!booted)
    return (
      <div className="auth-loading" role="status">
        Opening your basket…
      </div>
    )
  if (!session)
    return (
      <AuthScreen
        setupRequired={setupRequired}
        error={error}
        retry={() => void boot()}
        onSignedIn={(session, recoveryCode) => {
          setReady(false)
          setSession(session)
          write(cachedSession, session)
          setError('')
          setRecovery(recoveryCode ?? null)
        }}
      />
    )
  return (
    <Context.Provider
      value={{
        state,
        ready,
        session,
        status,
        pending,
        error,
        clearError: () => setError(''),
        update,
        action,
        signOut,
        refreshSession,
        retry: () => void refresh(),
        resolvePending,
      }}
    >
      {ready && <div data-app-ready="true">{children}</div>}
      {!ready && (
        <div className="auth-loading" role="status">
          Loading your household…
        </div>
      )}
      <Modal
        open={!!recovery}
        onOpenChange={(value) => {
          if (!value) setRecovery(null)
        }}
        title="Keep your recovery code"
        description="Save this code somewhere safe. It lets you reset your password without email delivery. It is shown only once."
      >
        <code className="recovery-code">{recovery}</code>
        <button
          className="primary-button"
          onClick={() => {
            const file = new Blob(
              [
                `Little Wardrobe recovery code\n${session.user.email}\n${recovery}\n`,
              ],
              { type: 'text/plain' },
            )
            const url = URL.createObjectURL(file)
            const link = document.createElement('a')
            link.href = url
            link.download = 'littlewardrobe-recovery.txt'
            link.click()
            URL.revokeObjectURL(url)
          }}
        >
          Download recovery code
        </button>
        <button className="secondary-button" onClick={() => setRecovery(null)}>
          I saved my code
        </button>
      </Modal>
    </Context.Provider>
  )
}
export function useApp() {
  const context = useContext(Context)
  if (!context) throw new Error('AppProvider is required')
  return {
    ...context,
    allRecipes: context.state.recipeBook
      ? [...context.state.recipeBook, ...context.state.customRecipes]
      : [...recipes, ...context.state.customRecipes],
    shoppingItems: getShoppingItems(context.state),
    groups: context.state.groups ?? [],
  }
}
