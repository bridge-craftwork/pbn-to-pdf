<script setup>
// The layouts, shown rather than named: a thumbnail of each one's first page,
// with a checkbox. Tick any number and generate them together.
//
// Previews render the opening boards only, so the whole gallery costs a
// fraction of one full lesson. They are rendered up front, before any tick,
// because recognising a layout is the point.
//
// All six sit on one row, which makes each thumbnail small — too small to read
// a diagram in. Clicking one asks the parent to enlarge it; the caption strip
// below is what ticks it.
import { layoutLabel, usesCardArt } from '@/lib/render.js'

const props = defineProps({
  layouts: { type: Array, required: true },   // [{ id, state, image, error, view }]
  selected: { type: Array, required: true },
})
const emit = defineEmits(['update:selected', 'enlarge'])

function toggle(id) {
  const next = new Set(props.selected)
  next.has(id) ? next.delete(id) : next.add(id)
  emit('update:selected', [...next])
}
</script>

<template>
  <ul class="gallery">
    <li
      v-for="l in layouts"
      :key="l.id"
      :class="{ on: selected.includes(l.id), failed: l.state === 'error' }"
    >
      <button
        class="shot"
        :class="l.state"
        type="button"
        :disabled="l.state !== 'ready'"
        :title="`Enlarge ${layoutLabel(l.id)}`"
        @click="emit('enlarge', l.id)"
      >
        <img v-if="l.state === 'ready'" :src="l.image" :alt="`${layoutLabel(l.id)} preview`" />
        <span v-else-if="l.state === 'error'" class="msg">{{ l.error }}</span>
        <span v-else class="msg muted">Rendering…</span>
        <span v-if="l.state === 'ready'" class="zoom" aria-hidden="true">⤢</span>
      </button>

      <label class="caption">
        <input
          type="checkbox"
          :checked="selected.includes(l.id)"
          :disabled="l.state !== 'ready'"
          @change="toggle(l.id)"
        />
        <span class="text">
          <span class="name">{{ layoutLabel(l.id) }}</span>
          <span class="meta muted">
            {{ l.view }}<template v-if="usesCardArt(l.id)"> · card art</template>
          </span>
        </span>
      </label>
    </li>
  </ul>
</template>

<style scoped>
.gallery {
  list-style: none; margin: 0; padding: 0;
  display: grid; gap: 0.6rem;
  grid-template-columns: repeat(6, minmax(0, 1fr));
}
@media (max-width: 1180px) { .gallery { grid-template-columns: repeat(3, minmax(0, 1fr)); } }
@media (max-width: 640px) { .gallery { grid-template-columns: repeat(2, minmax(0, 1fr)); } }

li {
  border: 1px solid var(--line);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--panel);
  display: flex; flex-direction: column;
}
li.on {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent);
}
li.failed { opacity: 0.7; }

.shot {
  /* A button, so reset the shared button chrome back to a bare image well. */
  border: 0; border-radius: 0; padding: 0; margin: 0; width: 100%;
  position: relative;
  display: flex; align-items: center; justify-content: center;
  /* Square, not page-shaped: five of the six previews are portrait and one is
   * landscape, and a square well wastes the least on either. */
  aspect-ratio: 1 / 1;
  background: #fff;
  border-bottom: 1px solid var(--line);
  overflow: hidden;
  cursor: zoom-in;
}
.shot:disabled { cursor: default; }
.shot img { max-width: 100%; max-height: 100%; display: block; }
.msg { font-size: 0.8rem; padding: 0.5rem; text-align: center; }
.shot.error { background: color-mix(in srgb, var(--danger) 10%, #fff); }
.shot.error .msg { color: var(--danger); }

.zoom {
  position: absolute; right: 0.25rem; bottom: 0.25rem;
  background: color-mix(in srgb, #000 55%, transparent);
  color: #fff; font-size: 0.75rem; line-height: 1;
  border-radius: 4px; padding: 0.2rem 0.3rem;
  opacity: 0; transition: opacity 0.12s;
}
.shot:hover .zoom, .shot:focus-visible .zoom { opacity: 1; }

.caption {
  display: flex; align-items: flex-start; gap: 0.4rem;
  margin: 0; padding: 0.4rem 0.5rem; cursor: pointer; font-weight: 400;
  flex: 1;
}
li.on .caption { background: color-mix(in srgb, var(--accent) 16%, transparent); }
li:has(input:disabled) .caption { cursor: default; }
.caption input { margin: 0.15rem 0 0; flex: none; }
.text { display: block; min-width: 0; }
.name { display: block; font-weight: 600; font-size: 0.85rem; line-height: 1.2; }
.meta { display: block; font-size: 0.75rem; margin-top: 0.15rem; }
</style>
