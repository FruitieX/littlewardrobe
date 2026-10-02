import { readFile } from 'node:fs/promises'
import { launchBrowser } from './browser.mjs'
const svg = await readFile('public/favicon.svg', 'utf8')
const browser = await launchBrowser()
try {
  const page = await browser.newPage()
  for (const size of [192, 512]) {
    await page.setViewportSize({ width: size, height: size })
    await page.setContent(
      `<style>body{margin:0;background:#244f8f}svg{width:100vw;height:100vh}</style>${svg}`,
    )
    await page.screenshot({ path: `public/icon-${size}.png` })
  }
} finally {
  await browser.close()
}
