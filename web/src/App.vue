<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import SourcePicker from '@/components/SourcePicker.vue'
import LayoutGallery from '@/components/LayoutGallery.vue'
import {
  DEFAULT_LESSON, VIEW_FOR_LAYOUT, fetchLibrary, fetchSetViews,
} from '@/lib/baker.js'
import {
  layoutLabel, layouts, orderLayouts, renderBytes, renderPreviewBytes, usesCardArt,
} from '@/lib/render.js'
import { firstPageImage } from '@/lib/thumbnail.js'
import { makeZip } from '@/lib/zip.js'

// Read here rather than in the template: Vite's `define` substitutes plain JS,
// but the Vue compiler resolves template identifiers against the component
// context, so `__APP_VERSION__` there stays unreplaced and renders empty.
const version = __APP_VERSION__

const source = ref(null)
const gallery = ref([])            // [{ id, state, image, error, view }]
const selected = ref([])
const circles = ref({
  circleSureWinners: false, circlePromotableWinners: false, circleLengthWinners: false,
})
const result = ref(null)           // { url, name, bytes, count }
const working = ref(false)
const error = ref('')
const engineReady = ref(false)
const resultPanel = ref(null)

// Every render of this selection is stale once the source or the options change.
let generation = 0

/// The PBN a layout should be rendered from. Library sources carry one per
/// rotation; a file or URL is a single document used for all of them.
function pbnFor(layout) {
  if (!source.value) return null
  if (source.value.kind !== 'library') return source.value.text
  return source.value.views?.[VIEW_FOR_LAYOUT[layout]] ?? null
}

const showCircles = computed(() => selected.value.some(usesCardArt))

const selectedLabels = computed(() =>
  orderLayouts(selected.value).map(layoutLabel).join(', '),
)

function revoke() {
  if (result.value) URL.revokeObjectURL(result.value.url)
  result.value = null
}
onBeforeUnmount(revoke)

async function buildGallery() {
  const mine = ++generation
  revoke()
  const ids = orderLayouts(await layouts())
  gallery.value = ids.map((id) => ({
    id, state: 'pending', image: null, error: '',
    view: source.value?.kind === 'library' ? VIEW_FOR_LAYOUT[id] : 'this file',
  }))
  // Keep any ticks that still make sense, so changing set does not clear them.
  selected.value = selected.value.filter((id) => ids.includes(id))

  for (const entry of gallery.value) {
    if (mine !== generation) return
    const pbn = pbnFor(entry.id)
    if (!pbn) {
      Object.assign(entry, { state: 'error', error: `No ${entry.view} rotation` })
      continue
    }
    try {
      const bytes = await renderPreviewBytes(pbn, entry.id, circles.value)
      const { url } = await firstPageImage(bytes)
      if (mine !== generation) return
      Object.assign(entry, { state: 'ready', image: url })
    } catch (e) {
      if (mine !== generation) return
      Object.assign(entry, { state: 'error', error: e.message || 'Could not render' })
    }
  }
}

onMounted(async () => {
  layouts()
    .then(() => (engineReady.value = true))
    .catch((e) => (error.value = `Could not load the renderer: ${e.message}`))
  try {
    // Open on a declarer-play lesson: its boards carry an opening lead, so the
    // declarer's plan has something to plan from, and the markers show well.
    const { lessons } = await fetchLibrary()
    const lesson = lessons.find((l) => l.name === DEFAULT_LESSON) ?? lessons[0]
    const set = lesson?.sets?.find((s) => s.id !== 'all') ?? lesson?.sets?.[0]
    if (!lesson || !set) return
    source.value = {
      kind: 'library', id: lesson.id, setId: set.id,
      name: `${lesson.name} — ${set.label}`,
      stem: `${lesson.name} ${set.label}`.replace(/[·]/g, '-'),
      views: await fetchSetViews(set),
    }
  } catch {
    // An opening example is a convenience; the picker still works without it.
  }
})

watch(source, (s) => {
  if (s) buildGallery()
})
// Circling changes what a preview looks like, so the thumbnails must follow.
watch(circles, () => source.value && buildGallery(), { deep: true })
watch(selected, revoke)

// A thumbnail on a six-across row is too small to read. Enlarging re-renders
// the same preview rather than upscaling the thumbnail: one preview is a few
// milliseconds, against holding six PDFs in memory on the chance of a click.
const zoom = ref(null)             // { id, url } | null
const zoomDialog = ref(null)

async function enlarge(id) {
  const pbn = pbnFor(id)
  if (!pbn) return
  closeZoom()
  zoom.value = { id, url: '' }
  await nextTick()
  zoomDialog.value?.showModal()
  try {
    const bytes = await renderPreviewBytes(pbn, id, circles.value)
    const blob = new Blob([bytes], { type: 'application/pdf' })
    if (zoom.value?.id === id) zoom.value = { id, url: URL.createObjectURL(blob) }
  } catch (e) {
    error.value = e.message || String(e)
    zoomDialog.value?.close()
  }
}

function closeZoom() {
  if (zoom.value?.url) URL.revokeObjectURL(zoom.value.url)
  zoom.value = null
}
onBeforeUnmount(closeZoom)

function toggleZoomed() {
  const id = zoom.value?.id
  if (!id) return
  const next = new Set(selected.value)
  next.has(id) ? next.delete(id) : next.add(id)
  selected.value = [...next]
}

async function generate() {
  if (!selected.value.length) return
  error.value = ''
  working.value = true
  revoke()
  try {
    const wanted = orderLayouts(selected.value)
    const files = []
    for (const layout of wanted) {
      const pbn = pbnFor(layout)
      if (!pbn) continue
      const bytes = await renderBytes(pbn, layout, circles.value)
      files.push({ name: `${source.value.stem} - ${layoutLabel(layout)}.pdf`, bytes })
    }
    if (!files.length) throw new Error('Nothing could be rendered from this selection.')

    if (files.length === 1) {
      const blob = new Blob([files[0].bytes], { type: 'application/pdf' })
      result.value = {
        url: URL.createObjectURL(blob), name: files[0].name,
        bytes: files[0].bytes.length, count: 1,
      }
    } else {
      const blob = makeZip(
        files,
        `${source.value.name}\n\n${files.map((f) => `  ${f.name}`).join('\n')}\n\n` +
          `Rendered by pbn-to-pdf v${version}.\n`,
      )
      result.value = {
        url: URL.createObjectURL(blob), name: `${source.value.stem}.zip`,
        bytes: blob.size, count: files.length,
      }
    }
    // The result is below the fold on a short window; take the reader there.
    await nextTick()
    resultPanel.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  } catch (e) {
    error.value = e.message || String(e)
  } finally {
    working.value = false
  }
}

const sizeLabel = computed(() =>
  result.value ? `${(result.value.bytes / 1024).toFixed(0)} KB` : '',
)
</script>

<template>
  <header>
    <h1>PBN to PDF</h1>
    <p class="muted">
      Bridge hand diagrams, declarer's plan worksheets and bidding sheets —
      rendered in your browser.
    </p>
    <a
      class="gh"
      href="https://github.com/bridge-craftwork/pbn-to-pdf"
      target="_blank"
      rel="noopener"
      title="pbn-to-pdf on GitHub"
    >
      <!-- The mark inline rather than fetched: the page loads no third-party
           asset, which is the same promise the tagline makes about the PBN. -->
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <path
          fill="currentColor"
          d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38
             0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13
             -.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66
             .07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15
             -.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.4 7.4 0 0 1 2-.27c.68 0
             1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82
             1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01
             1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z"
        />
      </svg>
      GitHub
    </a>
  </header>

  <main>
    <section class="panel steprow">
      <h2>1. Lesson</h2>
      <SourcePicker
        :loaded-id="source?.id ?? ''"
        :loaded-set-id="source?.setId ?? ''"
        @load="source = $event; error = ''"
        @error="error = $event"
      />
      <p v-if="source" class="loaded">
        <strong>{{ source.name }}</strong>
      </p>
      <p v-else class="muted loaded">Nothing loaded yet.</p>
    </section>

    <section class="panel">
      <div class="panelhead">
        <h2>2. Layouts</h2>
        <span class="muted hint">Click a preview to enlarge · tick to include</span>
        <span class="count" :class="{ none: !selected.length }">
          {{ selected.length }} selected
        </span>
      </div>

      <p v-if="!source" class="muted">Choose a lesson to see the layouts.</p>
      <template v-else>
        <LayoutGallery
          :layouts="gallery"
          :selected="selected"
          @update:selected="selected = $event"
          @enlarge="enlarge"
        />

        <div class="actions">
          <button class="primary" :disabled="!selected.length || working" @click="generate">
            {{ working ? 'Rendering…' : selected.length > 1
              ? `Generate ${selected.length} PDFs` : 'Generate PDF' }}
          </button>
          <span v-if="!engineReady" class="muted">Loading the renderer…</span>
          <span v-else-if="!selected.length" class="muted">Tick a layout above.</span>
          <span v-else class="muted picked">{{ selectedLabels }}</span>

          <fieldset v-if="showCircles">
            <legend>Circle winners</legend>
            <label><input v-model="circles.circleSureWinners" type="checkbox" /> Sure (red)</label>
            <label><input v-model="circles.circlePromotableWinners" type="checkbox" /> Promotable (green)</label>
            <label><input v-model="circles.circleLengthWinners" type="checkbox" /> Length (blue)</label>
          </fieldset>
        </div>
      </template>
    </section>

    <section v-if="error" class="panel"><p class="error">{{ error }}</p></section>

    <section v-if="result" ref="resultPanel" class="panel result">
      <div class="panelhead">
        <h2>
          3. {{ result.count > 1 ? `${result.count} PDFs` : 'Your PDF' }}
          <span class="muted">({{ sizeLabel }})</span>
        </h2>
        <a class="download" :href="result.url" :download="result.name">
          Download {{ result.count > 1 ? '.zip' : 'PDF' }}
        </a>
      </div>
      <iframe v-if="result.count === 1" :src="result.url" title="Rendered PDF"></iframe>
      <p v-else class="muted zipnote">{{ result.name }} — one PDF per layout you ticked.</p>
    </section>
  </main>

  <dialog ref="zoomDialog" class="modal wide tall" @close="closeZoom">
    <div class="modalhead">
      <h3>{{ zoom ? layoutLabel(zoom.id) : '' }} <span class="muted">— preview</span></h3>
      <div class="zoomactions">
        <button
          v-if="zoom"
          :class="{ primary: !selected.includes(zoom.id) }"
          @click="toggleZoomed"
        >
          {{ selected.includes(zoom.id) ? '✓ Selected — click to remove' : 'Select this layout' }}
        </button>
        <button class="ghost" aria-label="Close" @click="zoomDialog.close()">✕</button>
      </div>
    </div>
    <div class="modalbody fill zoombody">
      <iframe v-if="zoom?.url" :src="zoom.url" title="Layout preview"></iframe>
      <p v-else class="muted">Rendering…</p>
    </div>
  </dialog>

  <footer class="muted">
    <a href="https://github.com/bridge-craftwork/pbn-to-pdf" target="_blank" rel="noopener">pbn-to-pdf</a>
    v{{ version }} — public domain. Lessons from
    <a href="https://github.com/bridge-craftwork/Baker-Bridge" target="_blank" rel="noopener">Baker Bridge</a>.
    Native CLIs available in
    <a href="https://github.com/bridge-craftwork/pbn-to-pdf" target="_blank" rel="noopener">GitHub</a>.
  </footer>
</template>

<style scoped>
header, main, footer { max-width: 84rem; margin: 0 auto; padding: 0 1rem; }
header {
  display: flex; align-items: baseline; gap: 0.75rem; flex-wrap: wrap;
  padding-top: 0.9rem; padding-bottom: 0.6rem;
}
h1 { margin: 0; font-size: 1.25rem; }
header p { margin: 0; font-size: 0.88rem; }
.gh {
  margin-left: auto; align-self: center;
  display: inline-flex; align-items: center; gap: 0.3rem;
  color: var(--muted); text-decoration: none; font-size: 0.88rem;
  border: 1px solid var(--line); border-radius: var(--radius);
  padding: 0.2rem 0.5rem; white-space: nowrap;
}
.gh:hover { color: var(--ink); border-color: var(--muted); }
h2 { margin: 0 0 0.6rem; font-size: 1.05rem; }
main { display: grid; gap: 0.75rem; }
.panel {
  background: var(--panel); border: 1px solid var(--line);
  border-radius: var(--radius); padding: 0.8rem 0.9rem;
}

/* Step 1 is a single row: heading, the three ways in, and what is loaded. */
.steprow { display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; }
.steprow h2 { margin: 0; white-space: nowrap; }
.loaded {
  margin: 0; min-width: 0; padding: 0.25rem 0.6rem;
  border: 1px solid var(--line); border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  font-size: 0.92rem;
}

.panelhead {
  display: flex; align-items: baseline; gap: 0.75rem;
  flex-wrap: wrap; margin-bottom: 0.6rem;
}
.panelhead h2 { margin: 0; }
.hint { font-size: 0.82rem; }
.count {
  margin-left: auto; font-size: 0.85rem; font-weight: 600;
  color: var(--accent-ink); background: var(--accent);
  border-radius: 999px; padding: 0.1rem 0.55rem;
}
.count.none { color: var(--muted); background: transparent; font-weight: 400; }

fieldset {
  margin: 0; border: 1px solid var(--line);
  border-radius: var(--radius); padding: 0.15rem 0.6rem 0.35rem;
}
legend { font-weight: 600; padding: 0 0.3rem; font-size: 0.85rem; }
fieldset label {
  font-weight: 400; display: inline-flex; align-items: center;
  gap: 0.35rem; margin: 0 0.8rem 0 0; font-size: 0.88rem;
}
.actions { display: flex; align-items: center; gap: 0.75rem; margin-top: 0.7rem; flex-wrap: wrap; }
.picked { font-size: 0.85rem; }

/* `.panel a` below outranks a bare `.download`, which left the button's label
 * accent-on-accent — invisible. Match the panel rule's specificity. */
.panel a.download {
  background: var(--accent); color: var(--accent-ink); text-decoration: none;
  font-weight: 600; border-radius: var(--radius); padding: 0.45rem 0.9rem;
  white-space: nowrap; margin-left: auto;
}
iframe {
  width: 100%; height: min(72vh, 900px); border: 1px solid var(--line);
  border-radius: var(--radius); background: #fff;
}
.zoombody { padding: 0; }
.zoombody p { margin: auto; }
.zoombody iframe { flex: 1; height: auto; border: 0; border-radius: 0; }
.zoomactions { display: flex; align-items: center; gap: 0.5rem; }
.zipnote { margin: 0.5rem 0 0; }
footer { padding: 1.25rem 1rem 2rem; font-size: 0.85rem; }
footer a, .panel a { color: var(--accent); }
</style>
