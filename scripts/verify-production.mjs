import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { mkdtemp, rm, mkdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { resolve, join } from 'node:path'
import { createServer } from 'node:http'
import { randomUUID } from 'node:crypto'
import { launchBrowser } from './browser.mjs'

const directory = await mkdtemp(join(tmpdir(), 'littlewardrobe-browser-'))
await mkdir('mockups', { recursive: true })
let backend, browser
const errors = []
const calls = []
let editedImage = ''
let invalidExtraction = false
let holdPhotoRequests = false
const heldPhotoRequests = []
const provider = createServer(async (req, res) => {
  let raw = ''
  for await (const chunk of req) raw += chunk
  const body = raw ? JSON.parse(raw) : {}
  calls.push({ path: req.url, body })
  let result
  if (req.url.endsWith('/images/edits')) {
    editedImage = body.image[0].image_url.split(',')[1]
    result = { data: [{ b64_json: editedImage }] }
  } else if (req.url.endsWith('/models'))
    result = { data: [{ id: 'fixture-vision' }] }
  else {
    const prompt = body.messages?.[0]?.content ?? ''
    if (holdPhotoRequests && prompt.includes('cataloguing ONE'))
      await new Promise((resolve) => heldPhotoRequests.push(resolve))
    const tag =
      body.messages?.[1]?.content?.[0]?.text?.includes('\"tag\"') ?? false
    const content = prompt.includes('cataloguing ONE')
      ? {
          name: 'Striped cotton shirt',
          category: 'tops',
          brand: 'Example',
          size: tag ? '98' : '',
          sizeSystem: 'cm',
          color: 'Sage green',
          material: 'Cotton',
          season: invalidExtraction ? '' : 'all',
          condition: 'good',
          defects: 'Small mark on the sleeve.',
          notes: '',
          evidence: {
            size: tag
              ? 'Printed size 98 on the tag.'
              : 'The tag is not readable in this photo.',
            condition: 'Front photo; inspect the sleeve in person.',
          },
          uncertainFields: ['condition'],
          needsTagPhoto: !tag,
        }
      : {
          title: 'Lasten puuvillapaita 98',
          description:
            'Puuvillapaita, koko 98. Hyvä käytetty kunto. Hihassa pieni jälki.',
          price: 6,
          priceReason:
            'Unverified estimate (€4–8), not researched market data. Review against the actual condition.',
        }
    result = { choices: [{ message: { content: JSON.stringify(content) } }] }
  }
  res.writeHead(200, { 'content-type': 'application/json' })
  res.end(JSON.stringify(result))
})
await new Promise((r) => provider.listen(0, '127.0.0.1', r))
const fixtureUrl = `http://127.0.0.1:${provider.address().port}/v1`
const portServer = createServer()
await new Promise((r) => portServer.listen(0, '127.0.0.1', r))
const port = portServer.address().port
await new Promise((r) => portServer.close(r))
const base =
  process.env.LITTLEWARDROBE_BROWSER_URL ?? `http://127.0.0.1:${port}`
const token = 'isolated-browser-bootstrap-token'
const pause = (ms) => new Promise((r) => setTimeout(r, ms))
const waitFor = async (fn, label) => {
  for (let i = 0; i < 100; i++) {
    if (await fn()) return
    await pause(100)
  }
  throw new Error(`Timed out: ${label}`)
}
async function call(context, path, method = 'GET', body) {
  const r = await context.request.fetch(`${base}/api${path}`, {
    method,
    headers: { 'x-littlewardrobe-request': '1', origin: base },
    ...(body === undefined ? {} : { data: body }),
  })
  assert.equal(r.status(), 200, `${path}: ${await r.text()}`)
  return r.json()
}
async function patch(context, collection, key, before, after) {
  return call(context, '/operations', 'POST', {
    id: randomUUID(),
    operation: {
      type: 'patch',
      label: 'Browser fixture',
      changes: [{ collection, key, field: null, before, after }],
    },
  })
}
async function navigation(page, label) {
  await page
    .getByRole('navigation', { name: 'Main navigation' })
    .getByRole('button', { name: label, exact: true })
    .click()
  await page.waitForFunction(
    (label) =>
      [...document.querySelectorAll('nav button')].some(
        (button) =>
          button.textContent.trim() === label &&
          button.getAttribute('aria-current') === 'page',
      ),
    label,
  )
}
async function finishSync(context) {
  await waitFor(
    async () => (await context.request.get(`${base}/api/state`)).ok(),
    'server available',
  )
  await pause(300)
}
async function screenshot(page, name) {
  await page.evaluate(() => document.fonts.ready)
  await page.waitForFunction(() =>
    [...document.images].every((i) => i.complete),
  )
  await page.evaluate(() => window.scrollTo(0, 0))
  await page.screenshot({
    path: `mockups/${name}.png`,
    fullPage: true,
    animations: 'disabled',
  })
  if (name === 'wardrobe-mobile')
    await page.screenshot({
      path: 'mockups/wardrobe-mobile-viewport.png',
      animations: 'disabled',
    })
}
try {
  if (!process.env.LITTLEWARDROBE_BROWSER_URL) {
    if (!process.env.LITTLEWARDROBE_SERVER_EXECUTABLE)
      execFileSync(
        'cargo',
        ['build', '--manifest-path', 'backend/Cargo.toml'],
        { stdio: 'pipe' },
      )
    backend = spawn(
      process.env.LITTLEWARDROBE_SERVER_EXECUTABLE ??
        resolve('backend/target/debug/littlewardrobe-api'),
      [],
      {
        env: {
          ...process.env,
          LITTLEWARDROBE_BIND: `127.0.0.1:${port}`,
          LITTLEWARDROBE_ORIGIN: base,
          LITTLEWARDROBE_DATA_DIR: directory,
          LITTLEWARDROBE_STATIC_DIR:
            process.env.LITTLEWARDROBE_BROWSER_STATIC ?? resolve('dist/client'),
          LITTLEWARDROBE_SETUP_TOKEN: token,
        },
        stdio: 'pipe',
      },
    )
    backend.stderr.on('data', (b) => {
      if (String(b).includes('ERROR')) errors.push(String(b))
    })
  }
  await waitFor(async () => {
    try {
      return (await fetch(`${base}/api/ready`)).ok
    } catch {
      return false
    }
  }, 'production server startup')
  browser = await launchBrowser()
  const owner = await browser.newContext({
    viewport: { width: 1440, height: 1080 },
    permissions: ['clipboard-read', 'clipboard-write'],
    acceptDownloads: true,
  })
  const partner = await browser.newContext({
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
    acceptDownloads: true,
  })
  const stranger = await browser.newContext()
  assert.equal((await stranger.request.get(`${base}/api/state`)).status(), 401)
  const signIn = await stranger.newPage()
  await signIn.setViewportSize({ width: 390, height: 844 })
  await signIn.goto(base)
  await signIn
    .getByRole('heading', { name: 'Set up Little Wardrobe' })
    .waitFor()
  await screenshot(signIn, 'setup-mobile')
  await signIn.close()
  await call(owner, '/auth/setup', 'POST', {
    setupToken: token,
    email: 'owner@example.test',
    name: 'Sam',
    password: 'a long browser test password',
    householdName: 'Our family',
    timeZone: 'Europe/Helsinki',
    samples: false,
  })
  const invitation = await call(owner, '/invitations', 'POST', {
    email: 'partner@example.test',
  })
  const invite = new URL(invitation.url).searchParams.get('invite')
  await call(partner, '/auth/join', 'POST', {
    token: invite,
    email: 'partner@example.test',
    name: 'Kim',
    password: 'another long browser password',
  })
  const page = await owner.newPage(),
    mobile = await partner.newPage()
  page.setDefaultTimeout(12000)
  mobile.setDefaultTimeout(12000)
  page.on('pageerror', (e) => errors.push(e.message))
  mobile.on('pageerror', (e) => errors.push(e.message))
  await page.goto(base)
  await mobile.goto(base)
  await page.locator('[data-app-ready="true"]').first().waitFor()
  await mobile.locator('[data-app-ready="true"]').first().waitFor()
  await screenshot(mobile, 'empty-mobile')
  // Configure children and a storage box through the actual mobile interface.
  await navigation(mobile, 'Family')
  for (const [name, size, birthday, color] of [
    ['Alex', '104', '2022-02-01', 'sage'],
    ['Robin', '86', '2024-08-01', 'peach'],
  ]) {
    await mobile.getByRole('button', { name: 'Add child' }).click()
    const dialog = mobile.getByRole('dialog')
    await dialog.getByLabel('Child name').fill(name)
    await dialog.getByLabel('Birthday').fill(birthday)
    await dialog.getByLabel('Current clothing size (cm)').fill(size)
    await dialog.getByLabel('Child color').selectOption(color)
    await dialog.getByRole('button', { name: 'Save child' }).click()
    await waitFor(
      async () =>
        (await call(owner, '/state')).state.children.some(
          (c) => c.name === name,
        ),
      `save ${name}`,
    )
  }
  await mobile.getByRole('button', { name: 'Add location' }).click()
  await mobile.getByLabel('Location name').fill('Attic · winter box')
  await mobile.getByRole('button', { name: 'Save location' }).click()
  await waitFor(
    async () => (await call(owner, '/state')).state.locations.length === 1,
    'storage saved',
  )
  const config = {
    enabled: true,
    transport: 'api',
    baseUrl: fixtureUrl,
    model: 'fixture-vision',
    imageModel: 'fixture-edit',
    imageSize: '1024x1024',
    responseFormat: 'json',
    tokenParameter: 'omit',
    reasoningEffort: '',
    autoItemIcons: false,
  }
  await call(owner, '/ai/config', 'PUT', {
    config,
    apiKey: 'isolated-fixture-key',
  })
  await page.reload()
  await mobile.reload()
  // Deterministic illustrated test photos; these are never seeded in real households.
  const buffers = await page.evaluate(() => {
    function draw(tag) {
      const c = document.createElement('canvas')
      c.width = 640
      c.height = 640
      const x = c.getContext('2d')
      x.fillStyle = tag ? '#f8f7ee' : '#ece6da'
      x.fillRect(0, 0, 640, 640)
      if (tag) {
        x.fillStyle = '#2b4033'
        x.font = 'bold 75px sans-serif'
        x.textAlign = 'center'
        x.fillText('EXAMPLE', 320, 200)
        x.font = '160px sans-serif'
        x.fillText('98', 320, 400)
        x.font = '30px sans-serif'
        x.fillText('100% cotton', 320, 480)
      } else {
        x.fillStyle = '#9dae93'
        x.beginPath()
        x.moveTo(220, 140)
        x.lineTo(100, 220)
        x.lineTo(155, 320)
        x.lineTo(220, 280)
        x.lineTo(220, 520)
        x.lineTo(420, 520)
        x.lineTo(420, 280)
        x.lineTo(485, 320)
        x.lineTo(540, 220)
        x.lineTo(420, 140)
        x.quadraticCurveTo(320, 220, 220, 140)
        x.fill()
        x.save()
        x.clip()
        x.fillStyle = '#eef0e5'
        for (let y = 245; y < 500; y += 42) x.fillRect(85, y, 470, 12)
        x.restore()
      }
      return c.toDataURL('image/png').split(',')[1]
    }
    return [draw(false), draw(true)]
  })
  const photo = Buffer.from(buffers[0], 'base64'),
    tagPhoto = Buffer.from(buffers[1], 'base64')
  await navigation(mobile, 'Wardrobe')
  await mobile.getByRole('button', { name: 'Add clothes', exact: true }).click()
  let dialog = mobile.getByRole('dialog')
  await dialog
    .getByLabel('Upload clothing photo', { exact: true })
    .setInputFiles({ name: 'shirt.png', mimeType: 'image/png', buffer: photo })
  await dialog.locator('.photo-thumb').first().waitFor()
  await dialog
    .getByLabel('Name', { exact: true })
    .fill('My unsaved clothing draft')
  invalidExtraction = true
  await dialog
    .getByRole('button', { name: 'Suggest details from photos' })
    .click()
  await dialog
    .getByText('The AI returned an unsupported season.', { exact: false })
    .waitFor()
  assert.equal(
    await dialog.getByLabel('Name', { exact: true }).inputValue(),
    'My unsaved clothing draft',
  )
  assert.equal(await dialog.locator('.photo-thumb').count(), 1)
  invalidExtraction = false
  await dialog
    .getByRole('button', { name: 'Suggest details from photos' })
    .click()
  await dialog.getByText('Size wasn’t readable.', { exact: false }).waitFor()
  assert.equal(await dialog.getByLabel('Size on tag').inputValue(), '')
  await dialog
    .getByLabel('Upload tag photo', { exact: true })
    .setInputFiles({ name: 'tag.png', mimeType: 'image/png', buffer: tagPhoto })
  await waitFor(
    async () => (await dialog.locator('.photo-thumb').count()) === 2,
    'tag uploaded',
  )
  await dialog
    .getByRole('button', { name: 'Suggest details from photos' })
    .click()
  await waitFor(
    async () => (await dialog.getByLabel('Size on tag').inputValue()) === '98',
    'tag extraction',
  )
  await dialog.getByLabel('Worn by').selectOption({ label: 'Alex' })
  await dialog
    .getByLabel('Save for', { exact: true })
    .selectOption({ label: 'Robin' })
  await dialog
    .getByLabel('Storage location')
    .selectOption({ label: 'Attic · winter box' })
  await dialog.getByLabel('Where is it in its journey?').selectOption('active')
  await screenshot(mobile, 'intake-mobile')
  await dialog.getByRole('button', { name: 'Add to wardrobe' }).click()
  await page
    .locator('.garment-card')
    .filter({ hasText: 'Striped cotton shirt' })
    .waitFor()
  await finishSync(owner)
  assert.equal((await call(owner, '/state')).state.garments.length, 1)
  // A mobile edit syncs independently; image generation uses the original as its reference.
  await mobile
    .locator('.garment-card')
    .filter({ hasText: 'Striped cotton shirt' })
    .click()
  dialog = mobile.getByRole('dialog')
  await dialog
    .getByText('Prepare an optional edited photo', { exact: true })
    .click()
  await dialog.getByRole('button', { name: 'Create background edit' }).click()
  await waitFor(
    async () => (await dialog.locator('.photo-thumb').count()) === 3,
    'background-edit reference image',
  )
  await dialog.getByRole('button', { name: 'Save changes' }).click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].photos.length === 3,
    'edited photo saved',
  )
  const originalUrl = (await call(owner, '/state')).state.garments[0].photos[0]
    .url
  assert.equal(
    (await stranger.request.get(`${base}${originalUrl}`)).status(),
    401,
  )
  await navigation(page, 'Wardrobe')
  await page
    .locator('.garment-card')
    .filter({ hasText: 'Striped cotton shirt' })
    .click()
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Pass on to Robin' })
    .click()
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Save changes' })
    .click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].childId ===
      (await call(owner, '/state')).state.children.find(
        (c) => c.name === 'Robin',
      ).id,
    'hand-me-down saved',
  )
  // Editors keep the values originally shown, even after another device refreshes.
  await mobile.reload()
  await mobile.locator('[data-app-ready="true"]').first().waitFor()
  for (const device of [page, mobile]) {
    await device
      .locator('.garment-card')
      .filter({ hasText: 'Striped cotton shirt' })
      .click()
    await device
      .getByRole('dialog')
      .getByText('More details & purchase history', { exact: true })
      .click()
  }
  await mobile
    .getByRole('dialog')
    .getByLabel('Private notes')
    .fill('Other device note')
  await page.getByRole('dialog').getByLabel('Bought from').fill('Family sale')
  await mobile
    .getByRole('dialog')
    .getByRole('button', { name: 'Save changes' })
    .click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].notes ===
      'Other device note',
    'concurrent notes saved',
  )
  await page.waitForTimeout(400)
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Save changes' })
    .click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].purchaseSource ===
      'Family sale',
    'disjoint field saved',
  )
  assert.equal(
    (await call(owner, '/state')).state.garments[0].notes,
    'Other device note',
  )
  for (const device of [page, mobile]) {
    await device
      .locator('.garment-card')
      .filter({ hasText: 'Striped cotton shirt' })
      .click()
    await device
      .getByRole('dialog')
      .getByText('More details & purchase history', { exact: true })
      .click()
  }
  await mobile
    .getByRole('dialog')
    .getByLabel('Private notes')
    .fill('Winning note')
  await page
    .getByRole('dialog')
    .getByLabel('Private notes')
    .fill('Conflicting stale note')
  await mobile
    .getByRole('dialog')
    .getByRole('button', { name: 'Save changes' })
    .click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].notes === 'Winning note',
    'first shared field saved',
  )
  await page.waitForTimeout(400)
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Save changes' })
    .click()
  await page
    .getByRole('button', { name: /Review changes/ })
    .first()
    .click()
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Discard this change' })
    .click()
  await page.keyboard.press('Escape')
  assert.equal(
    (await call(owner, '/state')).state.garments[0].notes,
    'Winning note',
  )
  // Offline garment edits survive a production-shell reload and reconnect.
  await mobile.reload()
  await mobile.locator('[data-app-ready="true"]').first().waitFor()
  await mobile.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  await waitFor(
    async () =>
      await mobile.evaluate(() => !!navigator.serviceWorker.controller),
    'service worker control',
  )
  await partner.setOffline(true)
  await mobile
    .locator('.garment-card')
    .filter({ hasText: 'Striped cotton shirt' })
    .click()
  dialog = mobile.getByRole('dialog')
  await dialog
    .getByLabel('Name', { exact: true })
    .fill('Striped shirt · offline edit')
  await dialog.getByRole('button', { name: 'Save changes' }).click()
  await mobile.reload()
  await mobile
    .locator('.garment-card')
    .filter({ hasText: 'Striped shirt · offline edit' })
    .waitFor()
  await partner.setOffline(false)
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].name.includes(
        'offline edit',
      ),
    'offline reconnect',
  )
  await mobile
    .locator('.garment-card')
    .filter({ hasText: 'Striped shirt · offline edit' })
    .click()
  dialog = mobile.getByRole('dialog')
  await dialog.getByLabel('Where is it in its journey?').selectOption('sell')
  await dialog.getByRole('button', { name: 'Save changes' }).click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].status === 'sell',
    'ready to sell',
  )
  await navigation(mobile, 'Sell')
  await mobile.locator('.sell-selection input').first().check()
  await mobile.getByLabel('Listing platform').selectOption('vinted')
  await mobile.getByRole('button', { name: 'Suggest listing' }).click()
  dialog = mobile.getByRole('dialog')
  await dialog.getByLabel('Listing title').waitFor()
  assert.equal(await dialog.locator('.listing-photos label').count(), 2)
  await dialog.getByRole('button', { name: 'Approve listing' }).click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.saleListings[0]?.status ===
      'approved',
    'approval saved',
  )
  const listingId = (await call(owner, '/state')).state.saleListings[0].id
  assert.equal(
    (await call(owner, `/sales/${listingId}/export`)).listing.photos.length,
    1,
  )
  await screenshot(mobile, 'listing-mobile')
  const download = mobile.waitForEvent('download')
  await dialog.getByRole('button', { name: 'Download photos & text' }).click()
  const downloaded = await download
  assert.ok(downloaded.suggestedFilename())
  await dialog.getByLabel('Asking price').fill('7')
  assert.ok(
    await dialog.getByRole('button', { name: 'Approve listing' }).isVisible(),
  )
  await dialog.getByRole('button', { name: 'Save draft' }).click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.saleListings[0].status === 'draft',
    'edited approval reset',
  )
  await mobile.getByRole('button', { name: /Lasten puuvillapaita/ }).click()
  dialog = mobile.getByRole('dialog')
  await dialog.getByRole('button', { name: 'Approve listing' }).click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.saleListings[0].status === 'approved',
    'reapproval saved',
  )
  await dialog
    .getByLabel('Published listing link')
    .fill('https://www.vinted.fi/items/123-fixture-shirt')
  await dialog.getByRole('button', { name: 'Mark as listed' }).click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.saleListings[0].status === 'listed',
    'listing link saved',
  )
  mobile.once('dialog', (d) => d.accept('6.50'))
  await dialog.getByRole('button', { name: 'Record sale' }).click()
  await waitFor(
    async () =>
      (await call(owner, '/state')).state.garments[0].status === 'sold',
    'sale recorded',
  )
  assert.equal(
    (await call(owner, '/state')).state.saleListings[0].soldPrice,
    6.5,
  )
  await dialog.getByRole('button', { name: 'Close', exact: true }).click()
  // Fill a separate screenshot fixture inventory, and verify filters/bundles/backup.
  const snap = (await call(owner, '/state')).state
  const template = snap.garments[0]
  const names = [
    'Soft wool cardigan',
    'Everyday leggings',
    'Rainy-day jacket',
    'Cotton sleepsuit',
    'Little striped tee',
    'Winter knit hat',
    'Cozy fleece',
    'Playground trousers',
  ]
  for (let i = 0; i < names.length; i++) {
    const item = {
      ...template,
      id: randomUUID(),
      name: names[i],
      category: [
        'knitwear',
        'bottoms',
        'outerwear',
        'sleepwear',
        'tops',
        'accessories',
        'outerwear',
        'bottoms',
      ][i],
      brand: ['Example', 'Family find', 'Example', 'Little basics'][i % 4],
      size: ['104', '86', '110', '92'][i % 4],
      childId: snap.children[i % 2].id,
      nextChildId: '',
      status: i < 3 ? 'sell' : i % 2 ? 'active' : 'stored',
      photos: i % 3 ? [] : template.photos.filter((p) => p.kind === 'front'),
    }
    await patch(owner, 'garments', item.id, null, item)
  }
  await navigation(page, 'Wardrobe')
  await page.getByRole('button', { name: /Soft wool cardigan/ }).waitFor()
  await screenshot(page, 'wardrobe-desktop')
  await navigation(mobile, 'Wardrobe')
  await screenshot(mobile, 'wardrobe-mobile')
  await page.getByLabel('Search clothes').fill('cardigan')
  assert.equal(await page.locator('.garment-card').count(), 1)
  await page.getByLabel('Search clothes').fill('')
  await navigation(page, 'Family')
  await screenshot(page, 'family-desktop')
  await navigation(page, 'Sell')
  await page.locator('.sell-selection input').nth(0).check()
  await page.locator('.sell-selection input').nth(1).check()
  await page.getByRole('button', { name: 'Create draft', exact: true }).click()
  assert.ok(
    (await page.getByLabel('Listing title').inputValue()).includes('paketti'),
  )
  await page
    .getByRole('dialog')
    .getByRole('button', { name: 'Save draft' })
    .click()
  await waitFor(
    async () => (await call(owner, '/state')).state.saleListings.length === 2,
    'bundle saved',
  )
  await screenshot(page, 'sales-desktop')
  // Durable photo intake continues after page reload; a second device edits
  // the pending entry while the provider is deliberately held.
  await navigation(mobile, 'Wardrobe')
  await navigation(page, 'Wardrobe')
  const intake = mobile.getByRole('region', { name: 'Quick add clothes' })
  const beforeQuick = (await call(owner, '/state')).state
  const quickChild = beforeQuick.children.find((child) => child.name === 'Alex')
  const quickLocation = beforeQuick.locations.find(
    (location) => location.name === 'Attic · winter box',
  )
  await mobile
    .locator('.children-tabs')
    .getByRole('button', { name: 'Alex', exact: true })
    .click()
  await mobile
    .locator('.status-tabs')
    .getByRole('button', { name: 'In rotation', exact: true })
    .click()
  if (
    (await mobile
      .getByRole('button', { name: 'Filters', exact: true })
      .getAttribute('aria-expanded')) !== 'true'
  )
    await mobile.getByRole('button', { name: 'Filters', exact: true }).click()
  const quickFilters = mobile.locator('.filter-grid')
  await quickFilters.getByLabel('Category').selectOption('outerwear')
  await quickFilters.getByLabel('Size', { exact: true }).fill('110')
  await quickFilters.getByLabel('Season').selectOption('winter')
  await quickFilters.getByLabel('Location').selectOption(quickLocation.id)
  await intake.getByRole('button', { name: 'Quick add', exact: true }).click()
  let quickDialog = mobile.getByRole('dialog')
  assert.equal(
    await quickDialog.getByLabel('Quick add child').inputValue(),
    quickChild.id,
  )
  assert.equal(
    await quickDialog.getByLabel('Quick add location').inputValue(),
    quickLocation.id,
  )
  await quickDialog.getByText('Starting details', { exact: true }).click()
  assert.equal(
    await quickDialog.getByLabel('Category').inputValue(),
    'outerwear',
  )
  assert.equal(await quickDialog.getByLabel('Size on tag').inputValue(), '110')
  await quickDialog.getByLabel('Size system').selectOption('cm')
  await quickDialog.getByText('Starting details', { exact: true }).click()
  await quickDialog.getByRole('button', { name: 'Add photo options' }).click()
  await quickDialog
    .getByRole('button', { name: 'Choose from gallery' })
    .waitFor()
  assert.equal(
    await quickDialog
      .getByLabel('Add photo from gallery', { exact: true })
      .getAttribute('capture'),
    null,
  )
  holdPhotoRequests = true
  await quickDialog
    .getByLabel('Add photo from gallery', { exact: true })
    .setInputFiles({
      name: 'background-shirt.png',
      mimeType: 'image/png',
      buffer: photo,
    })
  await quickDialog.locator('.photo-thumb').first().waitFor()
  await quickDialog.getByLabel('Add photo', { exact: true }).setInputFiles({
    name: 'background-tag.png',
    mimeType: 'image/png',
    buffer: tagPhoto,
  })
  await waitFor(
    async () => (await quickDialog.locator('.photo-thumb').count()) === 2,
    'two quick-add photos uploaded',
  )
  await quickDialog.getByLabel('Photo 2 type').selectOption('tag')
  assert.equal(heldPhotoRequests.length, 0)
  assert.equal((await call(owner, '/state')).state.garments.length, 9)
  await screenshot(mobile, 'quick-add-mobile')
  await quickDialog.getByRole('button', { name: 'Close dialog' }).click()
  await intake.getByRole('button', { name: 'Quick add', exact: true }).click()
  quickDialog = mobile.getByRole('dialog')
  assert.equal(await quickDialog.locator('.photo-thumb').count(), 2)
  assert.equal(
    await quickDialog.getByLabel('Quick add child').inputValue(),
    quickChild.id,
  )
  await quickDialog
    .getByRole('button', { name: 'Add to wardrobe', exact: true })
    .click()
  await quickDialog.waitFor({ state: 'hidden' })
  await waitFor(
    async () => heldPhotoRequests.length === 1,
    'background provider started',
  )
  let pendingPhoto = (await call(owner, '/state')).state.garments.find(
    (g) => g.name === 'Photo awaiting details',
  )
  assert.ok(pendingPhoto?.aiReview)
  assert.equal(pendingPhoto.childId, quickChild.id)
  assert.equal(pendingPhoto.locationId, quickLocation.id)
  assert.equal(pendingPhoto.status, 'active')
  assert.equal(pendingPhoto.category, 'outerwear')
  assert.equal(pendingPhoto.size, '110')
  assert.equal(pendingPhoto.season, 'winter')
  assert.deepEqual(
    pendingPhoto.photos.map((photo) => photo.kind),
    ['front', 'tag'],
  )
  const pendingCard = mobile
    .locator('.garment-card')
    .filter({ hasText: 'Photo awaiting details' })
  await pendingCard.locator('.card-photo-progress .spin').waitFor()
  await pendingCard.getByText('AI · needs review', { exact: true }).waitFor()
  await screenshot(mobile, 'quick-add-pending-mobile')
  await intake.getByRole('button', { name: 'Quick add', exact: true }).click()
  const nextQuick = mobile.getByRole('dialog')
  assert.equal(await nextQuick.locator('.photo-thumb').count(), 0)
  assert.equal(
    await nextQuick.getByLabel('Quick add child').inputValue(),
    quickChild.id,
  )
  assert.equal(
    await nextQuick.getByLabel('Quick add location').inputValue(),
    quickLocation.id,
  )
  await nextQuick.getByRole('button', { name: 'Cancel', exact: true }).click()

  const queuedJob = (await call(owner, '/ai/photo-jobs')).find(
    (job) => job.garmentId === pendingPhoto.id,
  )
  // Repeating the same acceptance after a lost response never adds a duplicate.
  await call(partner, '/ai/photo-jobs', 'POST', {
    id: queuedJob.id,
    photos: pendingPhoto.photos,
    defaults: {
      childId: quickChild.id,
      locationId: quickLocation.id,
      status: 'active',
      category: 'outerwear',
      size: '110',
      sizeSystem: 'cm',
      season: 'winter',
    },
  })
  await mobile.reload()
  await mobile.locator('[data-app-ready="true"]').first().waitFor()
  await mobile
    .locator('.garment-card')
    .filter({ hasText: 'Photo awaiting details' })
    .waitFor()
  await patch(owner, 'garments', pendingPhoto.id, pendingPhoto, {
    ...pendingPhoto,
    brand: 'Manual brand',
  })
  holdPhotoRequests = false
  heldPhotoRequests.shift()()
  await waitFor(async () => {
    const garment = (await call(owner, '/state')).state.garments.find(
      (g) => g.id === pendingPhoto.id,
    )
    return (
      garment?.name === 'Striped cotton shirt' &&
      garment.brand === 'Manual brand' &&
      garment.category === 'outerwear' &&
      garment.size === '110' &&
      garment.season === 'winter' &&
      garment.childId === quickChild.id &&
      garment.locationId === quickLocation.id
    )
  }, 'background AI merges without overwriting manual brand')
  for (const device of [page, mobile]) {
    await device
      .getByRole('button', { name: /Manual brand Striped cotton shirt/ })
      .waitFor()
  }
  await mobile
    .getByRole('button', { name: /Manual brand Striped cotton shirt/ })
    .click()
  let backgroundDialog = mobile.getByRole('dialog')
  await backgroundDialog
    .getByText('AI draft · check the details', { exact: false })
    .waitFor()
  assert.equal(
    await backgroundDialog.getByLabel('Size on tag').inputValue(),
    '110',
  )
  await screenshot(mobile, 'background-photo-review-mobile')
  // Header and close control remain in the viewport at the bottom of the form.
  await backgroundDialog.locator('.modal-body').evaluate((body) => {
    body.scrollTop = body.scrollHeight
  })
  const headerBox = await backgroundDialog
    .locator('.modal-header')
    .boundingBox()
  const closeBox = await backgroundDialog
    .getByRole('button', { name: 'Close dialog' })
    .boundingBox()
  assert.ok(
    headerBox &&
      closeBox &&
      closeBox.y >= headerBox.y &&
      closeBox.y < headerBox.y + headerBox.height,
  )
  await backgroundDialog
    .getByRole('button', { name: 'Save changes', exact: true })
    .click()
  await waitFor(
    async () =>
      !(await call(owner, '/state')).state.garments.find(
        (g) => g.id === pendingPhoto.id,
      ).aiReview,
    'adult reviewed background draft',
  )

  async function submitQuickPhoto(name) {
    await intake.getByRole('button', { name: 'Quick add', exact: true }).click()
    const modal = mobile.getByRole('dialog')
    await modal
      .getByLabel('Add photo', { exact: true })
      .setInputFiles({ name, mimeType: 'image/png', buffer: photo })
    await modal.locator('.photo-thumb').first().waitFor()
    await modal
      .getByRole('button', { name: 'Add to wardrobe', exact: true })
      .click()
    await modal.waitFor({ state: 'hidden' })
  }
  invalidExtraction = true
  await submitQuickPhoto('failed-shirt.png')
  await waitFor(
    async () =>
      (await call(owner, '/ai/photo-jobs')).some(
        (job) => job.status === 'failed',
      ),
    'failed background task is durable',
  )
  await mobile.reload()
  await mobile.locator('[data-app-ready="true"]').first().waitFor()
  await mobile.getByText('Photo details failed', { exact: true }).waitFor()
  invalidExtraction = false
  await mobile.getByRole('button', { name: 'Retry AI', exact: true }).click()
  await waitFor(
    async () =>
      !(await call(owner, '/ai/photo-jobs')).some(
        (job) =>
          job.status === 'failed' ||
          job.status === 'running' ||
          job.status === 'queued',
      ),
    'background retry succeeded',
  )
  let retryPhoto = (await call(owner, '/state')).state.garments.find(
    (g) => g.aiReview,
  )
  assert.equal(retryPhoto.photos[0].kind, 'front')

  holdPhotoRequests = true
  await submitQuickPhoto('cancel-shirt.png')
  await waitFor(
    async () => heldPhotoRequests.length === 1,
    'cancelable background task started',
  )
  const canceledPhoto = (await call(owner, '/state')).state.garments.find(
    (g) => g.name === 'Photo awaiting details',
  )
  await mobile.getByRole('button', { name: 'Cancel AI', exact: true }).click()
  holdPhotoRequests = false
  heldPhotoRequests.shift()()
  await waitFor(
    async () =>
      !(await call(owner, '/ai/photo-jobs')).some(
        (job) => job.garmentId === canceledPhoto.id,
      ),
    'background cancel persisted',
  )
  await pause(250)
  assert.equal(
    (await call(owner, '/state')).state.garments.find(
      (g) => g.id === canceledPhoto.id,
    ).name,
    'Photo awaiting details',
  )
  // Remove only these isolated fixtures so the existing backup assertions stay exact.
  for (const id of [pendingPhoto.id, retryPhoto.id, canceledPhoto.id]) {
    const current = (await call(owner, '/state')).state.garments.find(
      (g) => g.id === id,
    )
    await patch(owner, 'garments', id, current, null)
  }
  const backupResponse = await owner.request.get(`${base}/api/backup`)
  assert.equal(backupResponse.status(), 200)
  const backup = await backupResponse.json()
  assert.equal(backup.state.garments.length, 9)
  assert.ok(backup.images.some((i) => i.sourceKind === 'enhanced'))
  // Restore into the same isolated household with a revision guard.
  const revision = (await call(owner, '/state')).revision
  await call(owner, '/restore', 'POST', { backup, expectedRevision: revision })
  await waitFor(
    async () => (await call(owner, '/state')).state.garments.length === 9,
    'backup restoration',
  )
  for (const width of [320, 390, 768, 1440]) {
    await page.setViewportSize({ width, height: 1000 })
    await navigation(page, 'Wardrobe')
    const overflow = await page.evaluate(() => ({
      width: innerWidth,
      scroll: document.documentElement.scrollWidth,
      nodes: [...document.querySelectorAll('body *')]
        .filter((e) => e.getBoundingClientRect().right > innerWidth + 1)
        .map((e) => ({
          tag: e.tagName,
          class: e.className,
          right: e.getBoundingClientRect().right,
        }))
        .slice(0, 20),
    }))
    assert.ok(
      overflow.scroll <= width,
      `horizontal overflow at ${width}: ${JSON.stringify(overflow)}`,
    )
  }
  assert.equal(errors.length, 0, errors.join('\n'))
  assert.ok(
    calls.some(
      (c) =>
        c.path.endsWith('/images/edits') &&
        c.body.image[0].image_url.startsWith('data:image/'),
    ),
  )
  await writeFile(
    'mockups/README.md',
    'These are real production browser screenshots using an isolated test household, deterministic illustrated clothing fixtures and a controlled AI provider. The running app starts empty. Regenerate with `nix develop --command npm run test:production`.\n',
  )
  console.log(
    'Production browser checks passed: two accounts, private photos, configurable children/storage, tag intake, filter-aware multi-photo Quick add, pending card spinners, background retry/cancel/review, fixed dialog headers, reviewed AI, image references, hand-me-downs, offline reload/reconnect, sale approvals/export/price changes, sale history, bundles, backup/restore and 320–1440px layouts.',
  )
} catch (error) {
  await mkdir('test-results', { recursive: true })
  if (browser)
    for (const [i, ctx] of browser.contexts().entries())
      for (const [j, p] of ctx.pages().entries())
        await p
          .screenshot({
            path: `test-results/failure-${i}-${j}.png`,
            fullPage: true,
          })
          .catch(() => {})
  throw error
} finally {
  for (const release of heldPhotoRequests) release()
  await browser?.close()
  backend?.kill('SIGTERM')
  await new Promise((r) => provider.close(r))
  await rm(directory, { recursive: true, force: true })
}
