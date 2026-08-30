<script setup>
// Three ways in: the Baker Bridge library, a local file, or a URL. Emits
// `load` with { name, text } whichever route the visitor takes.
import { onMounted, onBeforeUnmount, ref } from 'vue'
import { fetchCatalogue, fetchLesson, fetchPbn } from '@/lib/baker.js'

const emit = defineEmits(['load', 'error'])

const tab = ref('library')
const categories = ref([])
const lesson = ref('')
const catalogueError = ref('')
const loadingCatalogue = ref(true)
const url = ref('')
const busy = ref(false)
const dragging = ref(false)

let controller = new AbortController()
onBeforeUnmount(() => controller.abort())

onMounted(async () => {
  try {
    categories.value = await fetchCatalogue(controller.signal)
  } catch (e) {
    if (e.name !== 'AbortError') catalogueError.value = e.message
  } finally {
    loadingCatalogue.value = false
  }
})

const named = (id) => {
  for (const c of categories.value) {
    const hit = c.lessons.find((l) => l.id === id)
    if (hit) return hit.name
  }
  return id
}

async function pickLesson() {
  if (!lesson.value) return
  busy.value = true
  try {
    emit('load', {
      name: named(lesson.value),
      text: await fetchLesson(lesson.value, controller.signal),
    })
  } catch (e) {
    if (e.name !== 'AbortError') emit('error', e.message)
  } finally {
    busy.value = false
  }
}

async function loadUrl() {
  const target = url.value.trim()
  if (!target) return
  busy.value = true
  try {
    const text = await fetchPbn(target, controller.signal)
    emit('load', { name: target.split('/').pop() || target, text })
  } catch (e) {
    if (e.name !== 'AbortError') emit('error', e.message)
  } finally {
    busy.value = false
  }
}

async function takeFile(file) {
  if (!file) return
  try {
    emit('load', { name: file.name, text: await file.text() })
  } catch {
    emit('error', `Could not read ${file.name}.`)
  }
}

function onDrop(e) {
  dragging.value = false
  takeFile(e.dataTransfer?.files?.[0])
}
</script>

<template>
  <div class="picker">
    <div class="tabs" role="tablist">
      <button
        v-for="t in ['library', 'file', 'url']"
        :key="t"
        role="tab"
        :aria-selected="tab === t"
        :class="{ on: tab === t }"
        @click="tab = t"
      >
        {{ { library: 'Baker Bridge library', file: 'Open a file', url: 'From a URL' }[t] }}
      </button>
    </div>

    <div v-if="tab === 'library'" class="pane">
      <p v-if="loadingCatalogue" class="muted">Loading the lesson list…</p>
      <p v-else-if="catalogueError" class="error">{{ catalogueError }}</p>
      <template v-else>
        <label for="lesson">Lesson</label>
        <select id="lesson" v-model="lesson">
          <option value="" disabled>Choose a lesson…</option>
          <optgroup v-for="c in categories" :key="c.id" :label="c.name">
            <option v-for="l in c.lessons" :key="l.id" :value="l.id">{{ l.name }}</option>
          </optgroup>
        </select>
        <button class="primary" :disabled="!lesson || busy" @click="pickLesson">
          {{ busy ? 'Loading…' : 'Load lesson' }}
        </button>
        <p class="muted note">
          50 lessons from
          <a href="https://github.com/bridge-craftwork/Baker-Bridge" target="_blank" rel="noopener">
            Baker Bridge</a
          >, public domain.
        </p>
      </template>
    </div>

    <div
      v-else-if="tab === 'file'"
      class="pane drop"
      :class="{ dragging }"
      @dragover.prevent="dragging = true"
      @dragleave="dragging = false"
      @drop.prevent="onDrop"
    >
      <p>Drop a <code>.pbn</code> file here, or</p>
      <input
        id="file"
        type="file"
        accept=".pbn,text/plain"
        @change="takeFile($event.target.files?.[0])"
      />
      <p class="muted note">Nothing is uploaded — the file is read in this browser.</p>
    </div>

    <div v-else class="pane">
      <label for="url">PBN file URL</label>
      <input id="url" v-model="url" type="url" placeholder="https://…/hands.pbn" @keyup.enter="loadUrl" />
      <button class="primary" :disabled="!url.trim() || busy" @click="loadUrl">
        {{ busy ? 'Fetching…' : 'Fetch' }}
      </button>
      <p class="muted note">The server must allow cross-origin requests.</p>
    </div>
  </div>
</template>

<style scoped>
.tabs { display: flex; flex-wrap: wrap; gap: 0.35rem; margin-bottom: 0.75rem; }
.tabs button { padding: 0.35rem 0.7rem; font-size: 0.9rem; }
.tabs button.on { background: var(--accent); border-color: var(--accent); color: var(--accent-ink); }
.pane { display: grid; gap: 0.6rem; }
.pane button.primary { justify-self: start; }
.note { font-size: 0.85rem; margin: 0; }
.drop {
  border: 2px dashed var(--line);
  border-radius: var(--radius);
  padding: 1.25rem;
  text-align: center;
  justify-items: center;
}
.drop.dragging { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, transparent); }
a { color: var(--accent); }
</style>
