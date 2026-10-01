export class HttpError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message)
  }
}
export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers)
  if (init.body && !(init.body instanceof FormData))
    headers.set('Content-Type', 'application/json')
  if (init.method && !['GET', 'HEAD'].includes(init.method))
    headers.set('X-LittleWardrobe-Request', '1')
  const response = await fetch(`/api${path}`, {
    ...init,
    headers,
    credentials: 'same-origin',
    cache: 'no-store',
  })
  if (!response.ok) {
    const error = await response
      .json()
      .catch(() => ({ message: 'The request could not be completed.' }))
    throw new HttpError(
      response.status,
      error.message ?? 'The request could not be completed.',
    )
  }
  const content = await response.text()
  return content ? (JSON.parse(content) as T) : (undefined as T)
}
export function post<T>(path: string, body: unknown) {
  return api<T>(path, { method: 'POST', body: JSON.stringify(body) })
}
export async function uploadImage(file: File): Promise<string> {
  const body = new FormData()
  body.append('image', file)
  const result = await api<{ url: string }>('/images', { method: 'POST', body })
  return result.url
}
