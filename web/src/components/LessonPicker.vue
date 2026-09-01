<script setup>
// A filterable table of the Baker Bridge library: 50 lessons across 7
// categories. Emits `pick` with { lesson, set } once both are chosen.
//
// Lives inside the library modal, and stretches to fill it: the table is the
// one scrolling region on screen while it is open, which is the whole reason
// the picker moved out of the page.
//
// The set chooser sits above the table, not below it. Below, it was past 50
// rows of scrolling — the thing you came to change was the thing hardest to
// reach. And the sets are chips rather than a `<select>`: macOS renders a
// native pulldown of 25 sets as a full-height list across the whole screen,
// which is a worse popup than the modal it opens from.
import { computed, nextTick, onMounted, ref, watch } from 'vue'

const props = defineProps({
  lessons: { type: Array, required: true },
  busy: { type: Boolean, default: false },
  selectedId: { type: String, default: '' },
  selectedSetId: { type: String, default: '' },
})
const emit = defineEmits(['pick'])

const query = ref('')
const chosen = ref(null)
const setId = ref('')
const filter = ref(null)
const table = ref(null)

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
// the whole lesson; otherwise the first sliced set. Coming back to the lesson
// that is already loaded, keep the set that is loaded with it.
watch(chosen, (l) => {
  const keep = l?.id === props.selectedId
    && l?.sets?.some((s) => s.id === props.selectedSetId)
  setId.value = keep ? props.selectedSetId : (l?.sets?.[0]?.id ?? '')
})

// Open on whatever is already loaded, so reopening the library to change set
// does not start from a blank table.
onMounted(async () => {
  chosen.value = props.lessons.find((l) => l.id === props.selectedId) ?? null
  await nextTick()
  filter.value?.focus()
  table.value?.querySelector('tr.on')?.scrollIntoView({ block: 'center' })
})

const currentSet = computed(() => chosen.value?.sets?.find((s) => s.id === setId.value) ?? null)

// Sets are grouped by size; a flat strip of 25 numbered chips is unreadable.
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

// Double-click takes the default set: the common case is "that lesson, whole".
function chooseAndLoad(lesson) {
  choose(lesson)
  nextTick(load)
}
</script>

<template>
  <div class="picker">
    <!-- type=text, not type=search: Chrome's search field swallows Escape to
         clear itself, and Escape has to close the modal instead. -->
    <input
      id="lesson-filter"
      ref="filter"
      v-model="query"
      type="text"
      placeholder="Filter by lesson or category — e.g. “declarer”, “Stayman”"
    />

    <div class="chosen" :class="{ empty: !chosen }">
      <template v-if="chosen">
        <div class="sets">
          <span class="who">{{ chosen.name }}</span>
          <template v-for="[group, sets] in setGroups" :key="group">
            <!-- The whole-lesson row is one chip; naming its group as well as
                 labelling the chip would say the same thing twice. -->
            <span class="glabel muted">{{ group === 'Whole lesson' ? '' : group }}</span>
            <span class="chips">
              <button
                v-for="s in sets"
                :key="s.id"
                class="chip"
                :class="{ on: setId === s.id }"
                :title="`${s.label} — ${s.boards} boards`"
                @click="setId = s.id"
              >
                {{ s.group ? s.short : s.label }}
              </button>
            </span>
          </template>
        </div>
        <button class="primary load" :disabled="!currentSet || busy" @click="load">
          {{ busy ? 'Loading…' : 'Load' }}
        </button>
      </template>
      <span v-else class="muted">Pick a lesson below, then choose a set.</span>
    </div>

    <div ref="table" class="tablewrap">
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
            @dblclick="chooseAndLoad(l)"
            @keyup.enter="chooseAndLoad(l)"
          >
            <td>{{ l.name }}</td>
            <td class="num">{{ l.boards }}</td>
          </tr>
        </tbody>
      </table>
      <p v-if="!matches.length" class="muted empty">No lesson matches “{{ query }}”.</p>
    </div>
  </div>
</template>

<style scoped>
.picker { display: flex; flex-direction: column; gap: 0.6rem; flex: 1; min-height: 0; }

.chosen {
  display: flex; align-items: center; gap: 0.6rem;
  border: 1px solid var(--line); border-radius: var(--radius);
  background: color-mix(in srgb, var(--accent) 8%, transparent);
  padding: 0.45rem 0.55rem;
}
.chosen.empty { background: transparent; }
/* One row per set size, group labels aligned in their own column: with 25
   4-board sets the strip wraps, and an unaligned label is then hard to tie to
   the chips it names. */
.sets {
  flex: 1; min-width: 0;
  display: grid; grid-template-columns: max-content 1fr;
  align-items: center; gap: 0.25rem 0.5rem;
}
.who { grid-column: 1 / -1; font-weight: 600; }
.glabel { font-size: 0.78rem; white-space: nowrap; }
.chips { display: flex; flex-wrap: wrap; gap: 0.25rem; }
.chip {
  padding: 0.1rem 0.45rem; font-size: 0.85rem; line-height: 1.4;
  border-radius: 999px; min-width: 1.7rem;
}
.chip.on { background: var(--accent); border-color: var(--accent); color: var(--accent-ink); font-weight: 600; }
.load { flex: none; }

.tablewrap {
  flex: 1;
  min-height: 8rem;
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
</style>
