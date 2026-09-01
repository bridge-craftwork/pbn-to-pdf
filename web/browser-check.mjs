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

// The library, the default lesson and every preview: nothing here is reached
// without the manifest fetch, the wasm load and pdf.js all working.
await page.waitForFunction(
  () => document.querySelectorAll('.gallery img').length >= 6, null, { timeout: 300_000 })
check('every layout previews as a thumbnail', (await page.locator('.gallery img').count()) === 6)
check('the default lesson preloads', true, await page.textContent('.loaded strong'))
check('the version reaches the footer', /v\d+\.\d+\.\d+/.test(await page.textContent('footer')))

// The library is a modal now, so the page itself carries no lesson table.
check('the lesson table stays out of the page',
  (await page.locator('tbody tr:not(.group)').count()) === 0)
await page.click('button:has-text("Baker Bridge library")')
await page.waitForSelector('dialog[open] #lesson-filter', { timeout: 10_000 })
check('the library lists its lessons', (await page.locator('tbody tr:not(.group)').count()) > 40)

// Filtering is why the table exists; 50 lessons is too many to scan.
await page.fill('#lesson-filter', 'declarer')
await page.waitForTimeout(300)
const filtered = await page.locator('tbody tr:not(.group)').count()
check('the filter narrows the table', filtered > 0 && filtered < 50, `${filtered} rows for "declarer"`)
await page.fill('#lesson-filter', '')

// Sets are chips above the table, not a <select> below it: macOS draws a
// native pulldown of 25 sets as a full-screen list, and below 50 rows of
// scrolling the chooser was the hardest thing in the modal to reach.
check('the set chooser carries no native pulldown',
  (await page.locator('dialog[open] select').count()) === 0)
check('the set chooser sits above the lesson table',
  await page.evaluate(() => {
    const sets = document.querySelector('dialog[open] .sets')
    const table = document.querySelector('dialog[open] .tablewrap')
    return !!sets && !!table &&
      (sets.compareDocumentPosition(table) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0
  }))
check('the loaded set is the one shown as chosen',
  (await page.locator('dialog[open] .chip.on').textContent()).trim() === '1')

// Choosing a different slice of the same lesson, and loading it.
await page.locator('dialog[open] .chips .chip', { hasText: /^2$/ }).first().click()
await page.click('dialog[open] button:has-text("Load")')
await page.waitForSelector('dialog[open]', { state: 'detached', timeout: 30_000 })
check('a set chip loads that set', (await page.textContent('.loaded')).includes('set 2'))
check('closing the library leaves the page scroll-free',
  (await page.locator('dialog[open]').count()) === 0)

// Reloading redraws every preview; wait for them before ticking anything.
await page.waitForFunction(
  () => document.querySelectorAll('.gallery img').length >= 6, null, { timeout: 300_000 })

// Two layouts drawn from two different rotations, bundled together.
const tick = (name) =>
  page.locator('.gallery li').filter({ hasText: name }).locator('input[type=checkbox]')
await tick("Declarer's plan — 2 per page").check()
await tick('Bidding sheets').check()
check('the header counts what is ticked', (await page.textContent('.count')).includes('2'))

// A thumbnail on a six-across row is unreadably small; clicking one enlarges it.
await page.locator('.gallery .shot').first().click()
await page.waitForSelector('dialog[open] iframe', { timeout: 60_000 })
check('a preview enlarges into a modal', true)
await page.keyboard.press('Escape')
await page.waitForSelector('dialog[open]', { state: 'detached', timeout: 10_000 })
check('circling options appear for a declarer plan',
  (await page.locator('fieldset input').count()) === 3)

await page.click('button.primary:has-text("Generate")')
await page.waitForSelector('a.download', { timeout: 300_000 })
let name = await page.getAttribute('a.download', 'download')
const zip = await page.evaluate(async (u) => {
  const b = await (await fetch(u)).arrayBuffer()
  return { sig: new TextDecoder().decode(new Uint8Array(b, 0, 2)), bytes: b.byteLength }
}, await page.getAttribute('a.download', 'href'))
check('several layouts arrive as one zip', zip.sig === 'PK' && name.endsWith('.zip'),
  `${name}, ${(zip.bytes / 1024) | 0} KB`)

// One layout should not be wrapped in an archive.
await tick('Bidding sheets').uncheck()
await page.click('button.primary:has-text("Generate")')
await page.waitForSelector('a.download', { timeout: 300_000 })
name = await page.getAttribute('a.download', 'download')
const pdf = await page.evaluate(async (u) => {
  const b = await (await fetch(u)).arrayBuffer()
  return new TextDecoder().decode(new Uint8Array(b, 0, 5))
}, await page.getAttribute('a.download', 'href'))
check('a single layout downloads as a PDF', pdf === '%PDF-' && name.endsWith('.pdf'))

check('no console errors', problems.length === 0, problems.join(' | '))

await browser.close()
const failed = checks.filter((c) => !c).length
console.log(failed ? `\n${failed} check(s) failed` : '\nall browser checks passed')
process.exit(failed ? 1 : 0)
