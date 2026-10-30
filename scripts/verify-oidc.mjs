// Passkey (OIDC) sign-in against a mock provider, in the production build:
// the sign-in screen leads with the passkey, password sign-in stays as a
// fallback, and people outside the allowed groups are refused.
// Run after `npm run build`.
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { spawn } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import { mkdtemp, mkdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { launchBrowser } from './browser.mjs'

const data = await mkdtemp(join(tmpdir(), 'littlewardrobe-oidc-'))
const app = 'http://127.0.0.1:3410'
const idp = 'http://127.0.0.1:3412'
const logs = []
const b64url = (value) => Buffer.from(value).toString('base64url')

// Minimal OpenID provider: /authorize signs in whoever `user` is.
let user = {}
const codes = new Map()
const provider = createServer(async (req, res) => {
  const url = new URL(req.url, idp)
  const json = (body, status = 200) => {
    res.writeHead(status, { 'content-type': 'application/json' })
    res.end(JSON.stringify(body))
  }
  if (url.pathname === '/.well-known/openid-configuration')
    return json({
      issuer: idp,
      authorization_endpoint: `${idp}/authorize`,
      token_endpoint: `${idp}/token`,
    })
  if (url.pathname === '/authorize') {
    const q = url.searchParams
    assert.equal(q.get('client_id'), 'littlewardrobe')
    assert.equal(q.get('code_challenge_method'), 'S256')
    const code = randomUUID()
    codes.set(code, {
      ...user,
      nonce: q.get('nonce'),
      challenge: q.get('code_challenge'),
    })
    res.writeHead(302, {
      location: `${q.get('redirect_uri')}?code=${code}&state=${q.get('state')}`,
    })
    return res.end()
  }
  if (url.pathname === '/token' && req.method === 'POST') {
    let body = ''
    for await (const chunk of req) body += chunk
    const form = new URLSearchParams(body)
    const grant = codes.get(form.get('code'))
    codes.delete(form.get('code'))
    const challenge = createHash('sha256')
      .update(form.get('code_verifier') ?? '')
      .digest('base64url')
    if (
      !grant ||
      grant.challenge !== challenge ||
      form.get('client_secret') !== 'mock-secret'
    )
      return json({ error: 'invalid_grant' }, 400)
    const claims = {
      ...grant,
      challenge: undefined,
      iss: idp,
      aud: 'littlewardrobe',
      exp: Math.floor(Date.now() / 1000) + 60,
    }
    return json({
      id_token: `${b64url('{"alg":"none"}')}.${b64url(JSON.stringify(claims))}.`,
      token_type: 'Bearer',
    })
  }
  res.writeHead(404)
  res.end()
})
await new Promise((resolve) => provider.listen(3412, '127.0.0.1', resolve))

const backend = spawn(
  'cargo',
  ['run', '--manifest-path', 'backend/Cargo.toml'],
  {
    env: {
      ...process.env,
      LITTLEWARDROBE_BIND: '127.0.0.1:3410',
      LITTLEWARDROBE_STATIC_DIR: 'dist/client',
      LITTLEWARDROBE_ORIGIN: app,
      LITTLEWARDROBE_DATA_DIR: data,
      LITTLEWARDROBE_SETUP_TOKEN: 'oidc-only-bootstrap-token',
      LITTLEWARDROBE_SQLITE_JOURNAL_MODE: 'DELETE',
      LITTLEWARDROBE_OIDC_ISSUER: idp,
      LITTLEWARDROBE_OIDC_CLIENT_ID: 'littlewardrobe',
      LITTLEWARDROBE_OIDC_CLIENT_SECRET: 'mock-secret',
      LITTLEWARDROBE_OIDC_ALLOWED_GROUPS: 'family',
      LITTLEWARDROBE_OIDC_USER_MAP: 'sam:owner@example.test',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  },
)
backend.stdout.on('data', (chunk) => logs.push(chunk.toString()))
backend.stderr.on('data', (chunk) => logs.push(chunk.toString()))

let browser
try {
  const end = Date.now() + 120000
  for (;;) {
    try {
      if ((await fetch(`${app}/api/ready`)).ok) break
    } catch {}
    if (Date.now() > end) throw new Error('Server did not become ready')
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  const setup = await fetch(`${app}/api/auth/setup`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-littlewardrobe-request': '1',
      origin: app,
    },
    body: JSON.stringify({
      setupToken: 'oidc-only-bootstrap-token',
      email: 'owner@example.test',
      name: 'Sam',
      password: 'a sufficiently long password',
      householdName: 'Our family',
      timeZone: 'Europe/Helsinki',
      samples: false,
    }),
  })
  assert.equal(setup.status, 200)

  browser = await launchBrowser()
  const options = { viewport: { width: 390, height: 844 }, isMobile: true }
  const passkeyButton = (page) =>
    page.getByRole('link', { name: 'Sign in with passkey' })
  const me = (page) =>
    page.evaluate(async () => (await fetch('/api/auth/me')).json())
  await mkdir('test-results', { recursive: true })

  // The owner's password account is linked through the user map.
  user = {
    sub: 'sub-sam',
    preferred_username: 'sam',
    name: 'Sam',
    groups: ['admins', 'family'],
  }
  const owner = await (await browser.newContext(options)).newPage()
  await owner.goto(app)
  await passkeyButton(owner).waitFor()
  assert.equal(await owner.locator('input[type=password]').count(), 0)
  await owner.screenshot({ path: 'test-results/oidc-sign-in.png' })
  await passkeyButton(owner).click()
  await owner
    .locator('[data-app-ready="true"]')
    .first()
    .waitFor({ timeout: 30000 })
  assert.equal((await me(owner)).user.email, 'owner@example.test')
  const session = (await owner.context().cookies()).find(
    (cookie) => cookie.name === 'littlewardrobe_session',
  )
  assert.ok(session.expires > Date.now() / 1000 + 360 * 86400)

  // Someone outside the allowed groups is refused and told why.
  user = { sub: 'sub-kid', preferred_username: 'kid', groups: ['kids'] }
  const kid = await (await browser.newContext(options)).newPage()
  await kid.goto(app)
  await passkeyButton(kid).click()
  await kid.getByText('does not have access to Little Wardrobe').waitFor()
  assert.equal(new URL(kid.url()).search, '')
  assert.equal(
    await kid.evaluate(async () => (await fetch('/api/auth/me')).status),
    401,
  )

  // The password fallback still works, and the device remembers it.
  const secondDevice = await (await browser.newContext(options)).newPage()
  await secondDevice.goto(app)
  await secondDevice
    .getByRole('button', { name: 'Sign in with email and password' })
    .click()
  await secondDevice.getByLabel('Email').fill('owner@example.test')
  await secondDevice.getByLabel('Password').fill('a sufficiently long password')
  await secondDevice
    .getByRole('button', { name: 'Sign in', exact: true })
    .click()
  await secondDevice
    .locator('[data-app-ready="true"]')
    .first()
    .waitFor({ timeout: 30000 })
  await secondDevice.context().clearCookies()
  await secondDevice.goto(app)
  await secondDevice.getByLabel('Password').waitFor()
  await secondDevice
    .getByRole('button', { name: 'Use a passkey instead' })
    .waitFor()
  console.log('OIDC sign-in checks passed')
} catch (error) {
  console.error(logs.join(''))
  throw error
} finally {
  await browser?.close()
  backend.kill()
  provider.close()
  await rm(data, { recursive: true, force: true })
}
