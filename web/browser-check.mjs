// End-to-end check of the built site in a real browser.
//
// vitest covers the pure logic and wasm/verify.mjs covers the renderer, but
// neither proves the page wires them together — that the engine chunk loads,
// that a blob URL comes back, that switching layout clears a stale result.
//
//   npm run build:all && npm run preview &
//   node browser-check.mjs
//
// Not part of CI: it needs a Chromium on disk. Uses whatever Playwright or
// Chrome has already installed rather than downloading one.

import { existsSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { chromium } from 'playwright-core'

const URL_ = process.env.SITE_URL || 'http://localhost:4173/'

function findBrowser() {
  const cache = join(process.env.HOME, 'Library/Caches/ms-playwright')
  if (existsSync(cache)) {
    // Newest headless shell wins; the numeric suffix is Playwright's build id.
    const shells = readdirSync(cache)
      .filter((d) => d.startsWith('chromium_headless_shell-'))
      .sort((a, b) => Number(b.split('-')[1]) - Number(a.split('-')[1]))
    for (const dir of shells) {
      const base = join(cache, dir)
      for (const inner of readdirSync(base)) {
        const bin = join(base, inner, 'chrome-headless-shell')
        if (existsSync(bin)) return bin
      }
    }
  }
  const chrome = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
  if (existsSync(chrome)) return chrome
  throw new Error('No Chromium found. Install one, or set CHROME_PATH.')
}

const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || findBrowser() })
const page = await browser.newPage()
const problems = []
page.on('console', (m) => m.type() === 'error' && problems.push(m.text()))
page.on('pageerror', (e) => problems.push(`pageerror: ${e.message}`))

const checks = []
const check = (name, ok, detail = '') => {
  checks.push(ok)
  console.log(`  ${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`)
}

await page.goto(URL_, { waitUntil: 'networkidle', timeout: 120_000 })
check('page loads', (await page.title()).includes('PBN to PDF'))

// The example lesson arrives from Baker Bridge with no interaction.
await page.waitForSelector('.loaded strong', { timeout: 60_000 })
check('example lesson preloads', true, await page.textContent('.loaded strong'))

// Layout options come from the engine, so their presence proves the wasm ran.
await page.waitForFunction(
  () => document.querySelectorAll('#layout option').length > 1, null, { timeout: 240_000 })
const ids = await page.$$eval('#layout option', (n) => n.map((o) => o.value))
check('engine supplies all six layouts', ids.length === 6, ids.join(', '))

check("circle options shown for declarer's plan", (await page.locator('fieldset input').count()) === 3)

await page.click('button.primary:has-text("Render PDF")')
await page.waitForSelector('a.download', { timeout: 300_000 })
const href = await page.getAttribute('a.download', 'href')
const pdf = await page.evaluate(async (u) => {
  const b = await (await fetch(u)).arrayBuffer()
  return { head: new TextDecoder().decode(new Uint8Array(b, 0, 5)), bytes: b.byteLength }
}, href)
check('renders a real PDF', pdf.head === '%PDF-' && pdf.bytes > 10_000, `${(pdf.bytes / 1024) | 0} KB`)
check('download has a sensible filename', /\.pdf$/.test(await page.getAttribute('a.download', 'download')))

// A stale PDF under new settings is worse than none.
await page.selectOption('#layout', 'dealer-summary')
check('changing layout clears the result', (await page.locator('a.download').count()) === 0)
check('circle options hidden for other layouts', (await page.locator('fieldset input').count()) === 0)

check('no console errors', problems.length === 0, problems.join(' | '))

await browser.close()
const failed = checks.filter((c) => !c).length
console.log(failed ? `\n${failed} check(s) failed` : '\nall browser checks passed')
process.exit(failed ? 1 : 0)
