<script setup>
// Show a PDF from a blob URL. Desktop browsers get their own viewer in an
// iframe — search, zoom and print come with it. Touch browsers don't have one
// that works in a frame: iPadOS draws the PDF but the frame never scrolls (the
// swipe scrolls the page instead), and Android Chrome draws nothing at all. For
// those, pdf.js draws each page to a canvas in a box that scrolls.
//
// Pages are drawn only when they near the visible area, and a page far out of
// view gives its canvas back. An iPad caps the total canvas memory a tab may
// hold, and a long lesson drawn all at once at retina resolution exceeds it.
import { onBeforeUnmount, ref, watch } from 'vue'
import { pdfjs } from '@/lib/pdfjs.js'

const props = defineProps({
  url: { type: String, required: true },
  title: { type: String, default: 'PDF' },
})

// iPadOS reports itself as a Mac, so tell them apart by touch.
const isIOS =
  /iP(hone|ad|od)/.test(navigator.userAgent) ||
  (/Macintosh/.test(navigator.userAgent) && navigator.maxTouchPoints > 1)
const native = !isIOS && navigator.pdfViewerEnabled !== false

const box = ref(null)
const pages = ref([])            // [{ aspect }] — height / width of each page
const error = ref('')

let doc = null
let observer = null
let generation = 0
const drawn = new Map()          // page index -> in-flight or finished render task

async function load(url) {
  const mine = ++generation
  teardown()
  pages.value = []
  error.value = ''
  try {
    const data = new Uint8Array(await (await fetch(url)).arrayBuffer())
    const loaded = await pdfjs.getDocument({ data }).promise
    if (mine !== generation) return loaded.destroy()
    doc = loaded
    const sizes = []
    for (let i = 1; i <= doc.numPages; i++) {
      const v = (await doc.getPage(i)).getViewport({ scale: 1 })
      sizes.push({ aspect: v.height / v.width })
    }
    if (mine !== generation) return
    pages.value = sizes
  } catch (e) {
    if (mine === generation) error.value = `Could not show the PDF: ${e.message}`
  }
}

// Called by each page's template ref once its canvas exists.
function watchPage(el) {
  if (!el || !box.value) return
  observer ??= new IntersectionObserver(onIntersect, {
    root: box.value,
    rootMargin: '100% 0px',
  })
  observer.observe(el)
}

function onIntersect(entries) {
  for (const { target, isIntersecting } of entries) {
    const index = Number(target.dataset.page)
    if (isIntersecting) draw(target, index)
    else release(target, index)
  }
}

async function draw(canvas, index) {
  if (drawn.has(index) || !doc) return
  const task = { cancelled: false, render: null }
  drawn.set(index, task)
  const page = await doc.getPage(index + 1)
  if (task.cancelled) return
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  const scale = (canvas.clientWidth * dpr) / page.getViewport({ scale: 1 }).width
  const viewport = page.getViewport({ scale })
  canvas.width = Math.ceil(viewport.width)
  canvas.height = Math.ceil(viewport.height)
  const canvasContext = canvas.getContext('2d')
  canvasContext.fillStyle = '#fff'
  canvasContext.fillRect(0, 0, canvas.width, canvas.height)
  task.render = page.render({ canvasContext, viewport, canvas })
  try {
    await task.render.promise
  } catch {
    // Cancelled because the page scrolled away; release() already reset it.
  }
}

function release(canvas, index) {
  const task = drawn.get(index)
  if (!task) return
  task.cancelled = true
  task.render?.cancel()
  drawn.delete(index)
  // A zero-sized canvas is how a browser is told it may free the backing store.
  canvas.width = 0
  canvas.height = 0
}

function teardown() {
  observer?.disconnect()
  observer = null
  for (const task of drawn.values()) {
    task.cancelled = true
    task.render?.cancel()
  }
  drawn.clear()
  doc?.destroy()
  doc = null
}

if (!native) watch(() => props.url, load, { immediate: true })
onBeforeUnmount(() => {
  generation++
  teardown()
})
</script>

<template>
  <iframe v-if="native" class="pdfview" :src="url" :title="title"></iframe>
  <div v-else ref="box" class="pdfview pages" role="document" :aria-label="title">
    <p v-if="error" class="error">{{ error }}</p>
    <p v-else-if="!pages.length" class="muted">Loading…</p>
    <canvas
      v-for="(p, i) in pages"
      :key="`${url}#${i}`"
      :ref="watchPage"
      :data-page="i"
      :style="{ aspectRatio: `1 / ${p.aspect}` }"
    ></canvas>
  </div>
</template>

<style scoped>
.pdfview {
  width: 100%; height: min(72vh, 900px); border: 1px solid var(--line);
  border-radius: var(--radius); background: #fff;
}
.pages {
  overflow-y: auto; overscroll-behavior: contain;
  background: var(--line); padding: 0.5rem;
  display: flex; flex-direction: column; gap: 0.5rem;
}
.pages canvas { display: block; width: 100%; background: #fff; }
.pages p { margin: auto; }
</style>
