<script setup>
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import SourcePicker from '@/components/SourcePicker.vue'
import { DEFAULT_LESSON, fetchLesson } from '@/lib/baker.js'
import { layoutLabel, layouts, render, usesCardArt } from '@/lib/render.js'

const source = ref(null) // { name, text }
const layoutIds = ref([])
const layout = ref('declarers-plan-2up')
const circles = ref({
  circleSureWinners: false,
  circlePromotableWinners: false,
  circleLengthWinners: false,
})

const pdf = ref(null) // { url, bytes }
const rendering = ref(false)
const error = ref('')
const engineReady = ref(false)

const showCircles = computed(() => usesCardArt(layout.value))

function revoke() {
  if (pdf.value) URL.revokeObjectURL(pdf.value.url)
  pdf.value = null
}
onBeforeUnmount(revoke)

// A new file or a different layout invalidates whatever is on screen; leaving a
// stale PDF visible under new settings is worse than showing nothing.
watch([source, layout, circles], revoke, { deep: true })

onMounted(async () => {
  // Kick off the engine download and preload an example in parallel, so the
  // page has something to render the moment the wasm lands.
  layouts()
    .then((ids) => {
      layoutIds.value = ids
      engineReady.value = true
    })
    .catch((e) => (error.value = `Could not load the renderer: ${e.message}`))

  try {
    source.value = { name: DEFAULT_LESSON.name, text: await fetchLesson(DEFAULT_LESSON.id) }
  } catch {
    // An example is a convenience, not a requirement — the picker still works.
  }
})

function onLoad(loaded) {
  error.value = ''
  source.value = loaded
}

async function run() {
  if (!source.value) return
  error.value = ''
  rendering.value = true
  revoke()
  try {
    const { url, bytes } = await render(source.value.text, layout.value, circles.value)
    pdf.value = { url, bytes }
  } catch (e) {
    error.value = e.message || String(e)
  } finally {
    rendering.value = false
  }
}

const downloadName = computed(() => {
  const stem = (source.value?.name || 'hands').replace(/\.pbn$/i, '')
  return `${stem} - ${layoutLabel(layout.value).replace(/[—-]/g, '').replace(/\s+/g, ' ').trim()}.pdf`
})

const sizeLabel = computed(() =>
  pdf.value ? `${(pdf.value.bytes / 1024).toFixed(0)} KB` : '',
)
</script>

<template>
  <header>
    <h1>PBN to PDF</h1>
    <p class="muted">
      Bridge hand diagrams, declarer's plan worksheets and bidding sheets — rendered
      in your browser. Nothing is uploaded.
    </p>
  </header>

  <main>
    <section class="panel">
      <h2>1. Choose a PBN</h2>
      <SourcePicker @load="onLoad" @error="error = $event" />
      <p v-if="source" class="loaded">
        Loaded: <strong>{{ source.name }}</strong>
      </p>
    </section>

    <section class="panel">
      <h2>2. Choose a layout</h2>
      <label for="layout">Layout</label>
      <select id="layout" v-model="layout" :disabled="!layoutIds.length">
        <option v-for="id in layoutIds" :key="id" :value="id">{{ layoutLabel(id) }}</option>
        <option v-if="!layoutIds.length" :value="layout">{{ layoutLabel(layout) }}</option>
      </select>

      <fieldset v-if="showCircles">
        <legend>Circle winners</legend>
        <label><input v-model="circles.circleSureWinners" type="checkbox" /> Sure (red)</label>
        <label><input v-model="circles.circlePromotableWinners" type="checkbox" /> Promotable (green)</label>
        <label><input v-model="circles.circleLengthWinners" type="checkbox" /> Length (blue)</label>
      </fieldset>

      <button class="primary" :disabled="!source || rendering" @click="run">
        {{ rendering ? 'Rendering…' : 'Render PDF' }}
      </button>
      <p v-if="!engineReady" class="muted note">Loading the renderer…</p>
    </section>

    <section v-if="error" class="panel">
      <p class="error">{{ error }}</p>
    </section>

    <section v-if="pdf" class="panel result">
      <div class="resulthead">
        <h2>3. Your PDF <span class="muted">({{ sizeLabel }})</span></h2>
        <a class="download" :href="pdf.url" :download="downloadName">Download</a>
      </div>
      <iframe :src="pdf.url" title="Rendered PDF"></iframe>
    </section>
  </main>

  <footer class="muted">
    <a href="https://github.com/bridge-craftwork/pbn-to-pdf" target="_blank" rel="noopener">
      pbn-to-pdf</a
    >
    v{{ __APP_VERSION__ }} — public domain.
  </footer>
</template>

<style scoped>
header, main, footer { max-width: 60rem; margin: 0 auto; padding: 0 1rem; }
header { padding-top: 2rem; }
h1 { margin: 0 0 0.25rem; font-size: 1.6rem; }
h2 { margin: 0 0 0.75rem; font-size: 1.05rem; }
main { display: grid; gap: 1rem; padding-top: 1.5rem; }
.panel {
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  padding: 1rem;
}
.panel > button.primary { margin-top: 0.85rem; }
fieldset {
  margin: 0.85rem 0 0;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  padding: 0.6rem 0.8rem;
}
legend { font-weight: 600; padding: 0 0.3rem; }
fieldset label { font-weight: 400; display: flex; align-items: center; gap: 0.4rem; margin: 0.2rem 0; }
.loaded { margin: 0.85rem 0 0; }
.note { font-size: 0.85rem; margin: 0.5rem 0 0; }
.resulthead { display: flex; align-items: baseline; justify-content: space-between; gap: 1rem; }
.download {
  background: var(--accent);
  color: var(--accent-ink);
  text-decoration: none;
  font-weight: 600;
  border-radius: var(--radius);
  padding: 0.45rem 0.9rem;
}
iframe { width: 100%; height: min(75vh, 900px); border: 1px solid var(--line); border-radius: var(--radius); background: #fff; }
footer { padding: 2rem 1rem 3rem; font-size: 0.85rem; }
footer a { color: var(--accent); }
</style>
