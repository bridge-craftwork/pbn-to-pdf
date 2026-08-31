<script setup>
// Three ways in: the Baker Bridge library, a local file, or a URL. Emits
// `load` with a source object; the library route carries one PBN per rotation,
// the other two a single PBN used for every layout.
import { onBeforeUnmount, onMounted, ref } from 'vue'
import LessonPicker from '@/components/LessonPicker.vue'
import { fetchLibrary, fetchPbn, fetchSetViews } from '@/lib/baker.js'

const props = defineProps({ loadedId: { type: String, default: '' } })
const emit = defineEmits(['load', 'error'])

const tab = ref('library')
const lessons = ref([])
const libraryError = ref('')
const loadingLibrary = ref(true)
const busy = ref(false)
const url = ref('')
const dragging = ref(false)

const controller = new AbortController()
onBeforeUnmount(() => controller.abort())

onMounted(async () => {
  try {
    lessons.value = (await fetchLibrary(controller.signal)).lessons
  } catch (e) {
    if (e.name !== 'AbortError') libraryError.value = e.message
  } finally {
    loadingLibrary.value = false
  }
})

async function pick({ lesson, set }) {
  busy.value = true
  try {
    emit('load', {
      kind: 'library',
      id: lesson.id,
      name: `${lesson.name} — ${set.label}`,
      stem: `${lesson.name} ${set.label}`.replace(/[·]/g, '-'),
      views: await fetchSetViews(set, controller.signal),
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
    const name = decodeURIComponent(target.split('/').pop() || target)
    emit('load', { kind: 'url', id: target, name, stem: name.replace(/\.pbn$/i, ''), text })
  } catch (e) {
    if (e.name !== 'AbortError') emit('error', e.message)
  } finally {
    busy.value = false
  }
}

async function takeFile(file) {
  if (!file) return
  try {
    emit('load', {
      kind: 'file',
      id: file.name,
      name: file.name,
      stem: file.name.replace(/\.pbn$/i, ''),
      text: await file.text(),
    })
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
  <div>
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

    <div v-if="tab === 'library'">
      <p v-if="loadingLibrary" class="muted">Loading the lesson library…</p>
      <p v-else-if="libraryError" class="error">{{ libraryError }}</p>
      <LessonPicker
        v-else
        :lessons="lessons"
        :busy="busy"
        :selected-id="loadedId"
        @pick="pick"
      />
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
      <input type="file" accept=".pbn,text/plain" @change="takeFile($event.target.files?.[0])" />
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
  border: 2px dashed var(--line); border-radius: var(--radius);
  padding: 1.25rem; text-align: center; justify-items: center;
}
.drop.dragging { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, transparent); }
</style>
