<script setup>
// A filterable table of the Baker Bridge library: 50 lessons across 7
// categories. Emits `pick` with { lesson, set } once both are chosen.
import { computed, ref, watch } from 'vue'

const props = defineProps({
  lessons: { type: Array, required: true },
  busy: { type: Boolean, default: false },
  selectedId: { type: String, default: '' },
})
const emit = defineEmits(['pick'])

const query = ref('')
const chosen = ref(null)
const setId = ref('')

const matches = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return props.lessons
  // Match the category too, so "declarer" finds the whole group.
  return props.lessons.filter((l) =>
    `${l.name} ${l.categoryLabel}`.toLowerCase().includes(q),
  )
})

const grouped = computed(() => {
  const out = new Map()
  for (const l of matches.value) {
    if (!out.has(l.category)) out.set(l.category, { label: l.categoryLabel, rows: [] })
    out.get(l.category).rows.push(l)
  }
  return [...out.values()]
})

// Default to the first set with boards — for a lesson that offers it, that is
// the whole lesson; otherwise the first sliced set.
watch(chosen, (l) => {
  setId.value = l?.sets?.[0]?.id ?? ''
})

const currentSet = computed(() => chosen.value?.sets?.find((s) => s.id === setId.value) ?? null)

// Sets are grouped by size in the dropdown; a flat list of 20-odd is unreadable.
const setGroups = computed(() => {
  const out = new Map()
  for (const s of chosen.value?.sets ?? []) {
    const g = s.group ?? 'Whole lesson'
    if (!out.has(g)) out.set(g, [])
    out.get(g).push(s)
  }
  return [...out.entries()]
})

function choose(lesson) {
  chosen.value = lesson
}

function load() {
  if (chosen.value && currentSet.value) emit('pick', { lesson: chosen.value, set: currentSet.value })
}
</script>

<template>
  <div class="picker">
    <label for="lesson-filter">Filter</label>
    <input
      id="lesson-filter"
      v-model="query"
      type="search"
      placeholder="Lesson or category — e.g. “declarer”, “Stayman”"
    />

    <div class="tablewrap">
      <table>
        <thead>
          <tr><th>Lesson</th><th class="num">Boards</th></tr>
        </thead>
        <tbody v-for="g in grouped" :key="g.label">
          <tr class="group"><th colspan="2">{{ g.label }}</th></tr>
          <tr
            v-for="l in g.rows"
            :key="l.id"
            :class="{ on: chosen?.id === l.id, loaded: selectedId === l.id }"
            tabindex="0"
            @click="choose(l)"
            @keyup.enter="choose(l)"
          >
            <td>{{ l.name }}</td>
            <td class="num">{{ l.boards }}</td>
          </tr>
        </tbody>
      </table>
      <p v-if="!matches.length" class="muted empty">No lesson matches “{{ query }}”.</p>
    </div>

    <div v-if="chosen" class="chosen">
      <div class="setrow">
        <label :for="'set-' + chosen.id">Set</label>
        <select :id="'set-' + chosen.id" v-model="setId">
          <optgroup v-for="[group, sets] in setGroups" :key="group" :label="group">
            <option v-for="s in sets" :key="s.id" :value="s.id">
              {{ s.label }} ({{ s.boards }} boards)
            </option>
          </optgroup>
        </select>
      </div>
      <button class="primary" :disabled="!currentSet || busy" @click="load">
        {{ busy ? 'Loading…' : `Load ${chosen.name}` }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.picker { display: grid; gap: 0.6rem; }
.tablewrap {
  max-height: 17rem;
  overflow-y: auto;
  border: 1px solid var(--line);
  border-radius: var(--radius);
}
table { width: 100%; border-collapse: collapse; font-size: 0.92rem; }
thead th {
  position: sticky; top: 0; z-index: 1;
  background: var(--panel);
  text-align: left; padding: 0.4rem 0.6rem;
  border-bottom: 1px solid var(--line);
}
tr.group th {
  position: sticky; top: 1.9rem;
  background: color-mix(in srgb, var(--accent) 12%, var(--panel));
  text-align: left; padding: 0.3rem 0.6rem; font-size: 0.82rem;
  text-transform: uppercase; letter-spacing: 0.04em;
}
tbody tr:not(.group) { cursor: pointer; }
tbody tr:not(.group) td { padding: 0.32rem 0.6rem; border-bottom: 1px solid var(--line); }
tbody tr:not(.group):hover td { background: color-mix(in srgb, var(--accent) 8%, transparent); }
tbody tr.on td { background: color-mix(in srgb, var(--accent) 20%, transparent); font-weight: 600; }
tbody tr.loaded td:first-child::after { content: ' ●'; color: var(--accent); }
.num { text-align: right; color: var(--muted); width: 5rem; }
.empty { padding: 0.8rem; margin: 0; }
.chosen { display: flex; flex-wrap: wrap; gap: 0.6rem; align-items: end; }
.setrow { flex: 1 1 18rem; }
</style>
