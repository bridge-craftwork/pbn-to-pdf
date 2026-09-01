<script setup>
// Three ways in: the Baker Bridge library, a local file, or a URL. Emits
// `load` with a source object; the library route carries one PBN per rotation,
// the other two a single PBN used for every layout.
//
// The three routes are one row of buttons, and everything they need — the
// 50-lesson table, the URL box — opens in a modal. The picker is used once per
// visit and then never again, so it does not earn permanent screen space; the
// layouts and the PDF do.
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import LessonPicker from '@/components/LessonPicker.vue'
import { fetchLibrary, fetchPbn, fetchSetViews } from '@/lib/baker.js'

const props = defineProps({
  loadedId: { type: String, default: '' },
  loadedSetId: { type: String, default: '' },
})
const emit = defineEmits(['load', 'error'])

const lessons = ref([])
const libraryError = ref('')
const loadingLibrary = ref(true)
const busy = ref(false)
const url = ref('')
const dragging = ref(false)

const libDialog = ref(null)
const urlDialog = ref(null)
const libOpen = ref(false)
const fileInput = ref(null)

const controller = new AbortController()

onMounted(async () => {
  try {
    lessons.value = (await fetchLibrary(controller.signal)).lessons
  } catch (e) {
    if (e.name !== 'AbortError') libraryError.value = e.message
  } finally {
    loadingLibrary.value = false
  }
})

async function openLibrary() {
  libOpen.value = true
  await nextTick()
  libDialog.value?.showModal()
}

async function openUrl() {
  urlDialog.value?.showModal()
  await nextTick()
  urlDialog.value?.querySelector('input')?.focus()
}

async function pick({ lesson, set }) {
  busy.value = true
  try {
    emit('load', {
      kind: 'library',
      id: lesson.id,
      setId: set.id,
      name: `${lesson.name} — ${set.label}`,
      stem: `${lesson.name} ${set.label}`.replace(/[·]/g, '-'),
      views: await fetchSetViews(set, controller.signal),
    })
    libDialog.value?.close()
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
    urlDialog.value?.close()
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

// Dropping a file anywhere on the window still works, now that the drop zone
// is no longer a pane taking up room. dragenter/dragleave fire for every child
// element the pointer crosses, so count them rather than trusting one leave.
let depth = 0
const hasFile = (e) => [...(e.dataTransfer?.types ?? [])].includes('Files')

function onEnter(e) {
  if (!hasFile(e)) return
  depth += 1
  dragging.value = true
}
function onLeave() {
  depth = Math.max(0, depth - 1)
  if (!depth) dragging.value = false
}
function onOver(e) {
  if (hasFile(e)) e.preventDefault()
}
function onDrop(e) {
  if (!hasFile(e)) return
  e.preventDefault()
  depth = 0
  dragging.value = false
  takeFile(e.dataTransfer?.files?.[0])
}

window.addEventListener('dragenter', onEnter)
window.addEventListener('dragleave', onLeave)
window.addEventListener('dragover', onOver)
window.addEventListener('drop', onDrop)

onBeforeUnmount(() => {
  controller.abort()
  window.removeEventListener('dragenter', onEnter)
  window.removeEventListener('dragleave', onLeave)
  window.removeEventListener('dragover', onOver)
  window.removeEventListener('drop', onDrop)
})
</script>

<template>
  <div class="source">
    <div class="tabs">
      <button class="primary" @click="openLibrary">Baker Bridge library…</button>
      <button @click="fileInput.click()">Open a file…</button>
      <button @click="openUrl">From a URL…</button>
      <input
        ref="fileInput"
        class="hidden-input"
        type="file"
        accept=".pbn,text/plain"
        @change="takeFile($event.target.files?.[0]); $event.target.value = ''"
      />
    </div>

    <dialog ref="libDialog" class="modal wide tall" @close="libOpen = false">
      <div class="modalhead">
        <h3>Baker Bridge library</h3>
        <button class="ghost" aria-label="Close" @click="libDialog.close()">✕</button>
      </div>
      <div class="modalbody fill">
        <p v-if="loadingLibrary" class="muted">Loading the lesson library…</p>
        <p v-else-if="libraryError" class="error">{{ libraryError }}</p>
        <LessonPicker
          v-else-if="libOpen"
          :lessons="lessons"
          :busy="busy"
          :selected-id="loadedId"
          :selected-set-id="loadedSetId"
          @pick="pick"
        />
      </div>
    </dialog>

    <dialog ref="urlDialog" class="modal">
      <div class="modalhead">
        <h3>Load a PBN from a URL</h3>
        <button class="ghost" aria-label="Close" @click="urlDialog.close()">✕</button>
      </div>
      <div class="modalbody urlbody">
        <label for="pbn-url">PBN file URL</label>
        <input
          id="pbn-url"
          v-model="url"
          type="url"
          placeholder="https://…/hands.pbn"
          @keyup.enter="loadUrl"
        />
        <p class="muted note">The server must allow cross-origin requests.</p>
        <div class="modalfoot">
          <button @click="urlDialog.close()">Cancel</button>
          <button class="primary" :disabled="!url.trim() || busy" @click="loadUrl">
            {{ busy ? 'Fetching…' : 'Fetch' }}
          </button>
        </div>
      </div>
    </dialog>

    <Teleport to="body">
      <div v-if="dragging" class="dropveil">
        <p>Drop a <code>.pbn</code> file to load it</p>
        <p class="muted">Nothing is uploaded — the file is read in this browser.</p>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.source { display: contents; }
.tabs { display: flex; flex-wrap: wrap; gap: 0.4rem; }
.tabs button { padding: 0.4rem 0.75rem; font-size: 0.92rem; }
.hidden-input { display: none; }

.urlbody { display: grid; gap: 0.5rem; }
.note { font-size: 0.85rem; margin: 0; }
.modalfoot { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 0.5rem; }

.dropveil {
  position: fixed; inset: 0; z-index: 50;
  display: grid; align-content: center; justify-items: center; gap: 0.3rem;
  background: color-mix(in srgb, var(--bg) 88%, transparent);
  border: 3px dashed var(--accent);
  font-size: 1.1rem;
  pointer-events: none;
}
</style>
