<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
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
      kind: 'library', id: lesson.id,
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
      rendered in your browser. Nothing is uploaded.
    </p>
  </header>

  <main>
    <section class="panel">
      <h2>1. Choose a lesson</h2>
      <SourcePicker
        :loaded-id="source?.id ?? ''"
        @load="source = $event; error = ''"
        @error="error = $event"
      />
      <p v-if="source" class="loaded">Loaded: <strong>{{ source.name }}</strong></p>
    </section>

    <section class="panel">
      <h2>2. Pick your layouts</h2>
      <p v-if="!source" class="muted">Choose a lesson to see the layouts.</p>
      <template v-else>
        <LayoutGallery :layouts="gallery" v-model:selected="selected" />

        <fieldset v-if="showCircles">
          <legend>Circle winners on the declarer's plan</legend>
          <label><input v-model="circles.circleSureWinners" type="checkbox" /> Sure (red)</label>
          <label><input v-model="circles.circlePromotableWinners" type="checkbox" /> Promotable (green)</label>
          <label><input v-model="circles.circleLengthWinners" type="checkbox" /> Length (blue)</label>
        </fieldset>

        <div class="actions">
          <button class="primary" :disabled="!selected.length || working" @click="generate">
            {{ working ? 'Rendering…' : selected.length > 1
              ? `Generate ${selected.length} PDFs` : 'Generate PDF' }}
          </button>
          <span v-if="!engineReady" class="muted">Loading the renderer…</span>
          <span v-else-if="!selected.length" class="muted">Tick a layout above.</span>
        </div>
      </template>
    </section>

    <section v-if="error" class="panel"><p class="error">{{ error }}</p></section>

    <section v-if="result" class="panel result">
      <div class="resulthead">
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

  <footer class="muted">
    <a href="https://github.com/bridge-craftwork/pbn-to-pdf" target="_blank" rel="noopener">pbn-to-pdf</a>
    v{{ version }} — public domain. Lessons from
    <a href="https://github.com/bridge-craftwork/Baker-Bridge" target="_blank" rel="noopener">Baker Bridge</a>.
  </footer>
</template>

<style scoped>
header, main, footer { max-width: 64rem; margin: 0 auto; padding: 0 1rem; }
header { padding-top: 2rem; }
h1 { margin: 0 0 0.25rem; font-size: 1.6rem; }
h2 { margin: 0 0 0.75rem; font-size: 1.05rem; }
main { display: grid; gap: 1rem; padding-top: 1.5rem; }
.panel {
  background: var(--panel); border: 1px solid var(--line);
  border-radius: var(--radius); padding: 1rem;
}
fieldset {
  margin: 1rem 0 0; border: 1px solid var(--line);
  border-radius: var(--radius); padding: 0.6rem 0.8rem;
}
legend { font-weight: 600; padding: 0 0.3rem; }
fieldset label {
  font-weight: 400; display: inline-flex; align-items: center;
  gap: 0.4rem; margin: 0.2rem 1rem 0.2rem 0;
}
.actions { display: flex; align-items: center; gap: 0.75rem; margin-top: 1rem; flex-wrap: wrap; }
.loaded { margin: 0.85rem 0 0; }
.resulthead { display: flex; align-items: baseline; justify-content: space-between; gap: 1rem; }
.download {
  background: var(--accent); color: var(--accent-ink); text-decoration: none;
  font-weight: 600; border-radius: var(--radius); padding: 0.45rem 0.9rem; white-space: nowrap;
}
iframe {
  width: 100%; height: min(70vh, 820px); border: 1px solid var(--line);
  border-radius: var(--radius); background: #fff;
}
.zipnote { margin: 0.5rem 0 0; }
footer { padding: 2rem 1rem 3rem; font-size: 0.85rem; }
footer a, .panel a { color: var(--accent); }
</style>
