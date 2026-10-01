import { chromium } from 'playwright'

export const baseUrl = process.env.LITTLEWARDROBE_URL ?? 'http://127.0.0.1:3101'
export async function launchBrowser() {
  return chromium.launch({
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || undefined,
    args: ['--no-sandbox'],
    headless: true,
  })
}
export async function settle(page) {
  if (!page.url().startsWith('file:'))
    await page.locator('[data-app-ready="true"]').waitFor()
  await page.evaluate(() => document.fonts.ready)
  await page.waitForFunction(() =>
    [...document.images].every((img) => img.complete),
  )
  await page.waitForTimeout(300)
}
