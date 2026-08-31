<script setup>
// The layouts, shown rather than named: a thumbnail of each one's first page,
// with a checkbox. Tick any number and generate them together.
//
// Previews render the opening boards only, so the whole gallery costs a
// fraction of one full lesson. They are rendered up front, before any tick,
// because recognising a layout is the point.
import { computed } from 'vue'
import { layoutLabel, usesCardArt } from '@/lib/render.js'

const props = defineProps({
  layouts: { type: Array, required: true },   // [{ id, state, image, error, view }]
  selected: { type: Array, required: true },
})
const emit = defineEmits(['update:selected'])

const chosen = computed({
  get: () => props.selected,
  set: (v) => emit('update:selected', v),
})

function toggle(id) {
  const next = new Set(props.selected)
  next.has(id) ? next.delete(id) : next.add(id)
  chosen.value = [...next]
}
</script>

<template>
  <ul class="gallery">
    <li
      v-for="l in layouts"
      :key="l.id"
      :class="{ on: selected.includes(l.id), failed: l.state === 'error' }"
    >
      <label>
        <span class="shot" :class="l.state">
          <img v-if="l.state === 'ready'" :src="l.image" :alt="`${layoutLabel(l.id)} preview`" />
          <span v-else-if="l.state === 'error'" class="msg">{{ l.error }}</span>
          <span v-else class="msg muted">Rendering…</span>
        </span>
        <span class="row">
          <input
            type="checkbox"
            :checked="selected.includes(l.id)"
            :disabled="l.state !== 'ready'"
            @change="toggle(l.id)"
          />
          <span class="name">{{ layoutLabel(l.id) }}</span>
        </span>
        <span class="meta muted">
          {{ l.view }}<template v-if="usesCardArt(l.id)"> · card art</template>
        </span>
      </label>
    </li>
  </ul>
</template>

<style scoped>
.gallery {
  list-style: none; margin: 0; padding: 0;
  display: grid; gap: 0.75rem;
  grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
}
li {
  border: 1px solid var(--line);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--panel);
}
li.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent) inset; }
li.failed { opacity: 0.7; }
label { display: block; cursor: pointer; font-weight: 400; margin: 0; }
li:has(input:disabled) label { cursor: default; }
.shot {
  display: flex; align-items: center; justify-content: center;
  /* Roughly letter-shaped, so a page looks like a page while it loads. */
  aspect-ratio: 5 / 4;
  background: #fff;
  border-bottom: 1px solid var(--line);
  overflow: hidden;
}
.shot img { max-width: 100%; max-height: 100%; display: block; }
.msg { font-size: 0.8rem; padding: 0.5rem; text-align: center; }
.shot.error { background: color-mix(in srgb, var(--danger) 10%, #fff); }
.shot.error .msg { color: var(--danger); }
.row { display: flex; align-items: center; gap: 0.45rem; padding: 0.5rem 0.6rem 0.15rem; }
.name { font-weight: 600; font-size: 0.9rem; line-height: 1.2; }
.meta { display: block; padding: 0 0.6rem 0.55rem 1.85rem; font-size: 0.78rem; }
</style>
