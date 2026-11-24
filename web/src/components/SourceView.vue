<script setup>
// The PBN beside the PDF, syntax-coloured, as Bridge Viewer's View Source has
// it. Each board's record is a block: click one to send the PDF to its page.
// The text is split at the records `sourceRecords` reports, so what is a board
// here is exactly what the renderer took as one.
import { computed, nextTick, ref, watch } from 'vue'

const props = defineProps({
  text: { type: String, required: true },
  records: { type: Array, default: () => [] },   // [[start, end]]
  pages: { type: Array, default: () => [] },     // page per record, 0 if none
  highlighted: { type: Number, default: -1 },
})
const emit = defineEmits(['select'])

const box = ref(null)

/// The text as runs: the record blocks, and whatever lies between them.
const runs = computed(() => {
  const out = []
  let at = 0
  props.records.forEach(([start, end], index) => {
    if (start > at) out.push({ index: -1, html: colour(props.text.slice(at, start)) })
    out.push({ index, html: colour(props.text.slice(start, end)) })
    at = Math.max(at, end)
  })
  if (at < props.text.length) out.push({ index: -1, html: colour(props.text.slice(at)) })
  return out
})

const escape = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/// One line at a time: tag pairs, `%` and `;` comments, `{commentary}` and,
/// in section data, cards and annotations.
function colour(text) {
  let inBrace = false
  return text
    .split('\n')
    .map((line) => {
      if (inBrace) {
        const close = line.indexOf('}')
        if (close < 0) return `<span class="c">${escape(line)}</span>`
        inBrace = false
        return `<span class="c">${escape(line.slice(0, close + 1))}</span>${data(line.slice(close + 1))}`
      }
      if (/^[%;]/.test(line)) return `<span class="m">${escape(line)}</span>`
      const tag = /^(\s*)\[(\w+)(\s+)("(?:[^"\\]|\\.)*")?(\s*\])(.*)$/.exec(line)
      if (tag) {
        const [, lead, name, gap, value = '', close, rest] = tag
        return `${lead}<span class="d">[</span><span class="t">${escape(name)}</span>${gap}` +
          `<span class="v">${escape(value)}</span><span class="d">${escape(close.trim())}</span>${data(rest)}`
      }
      return data(line)
    })
    .join('\n')

  function data(line) {
    let out = ''
    let rest = line
    while (rest) {
      const open = rest.indexOf('{')
      const plain = open < 0 ? rest : rest.slice(0, open)
      out += escape(plain)
        .replace(/(^|[^A-Za-z0-9])([HD][2-9TJQKA])(?![A-Za-z0-9])/g, '$1<span class="r">$2</span>')
        .replace(/(=\d+=|\$\d+)/g, '<span class="a">$1</span>')
      if (open < 0) break
      const close = rest.indexOf('}', open)
      if (close < 0) {
        inBrace = true
        out += `<span class="c">${escape(rest.slice(open))}</span>`
        break
      }
      out += `<span class="c">${escape(rest.slice(open, close + 1))}</span>`
      rest = rest.slice(close + 1)
    }
    return out
  }
}

// Follow the highlighted record when it moves.
watch(
  () => props.highlighted,
  async (index) => {
    await nextTick()
    box.value?.querySelector(`[data-record="${index}"]`)?.scrollIntoView({ block: 'nearest' })
  },
)

const pageLabel = (index) => (props.pages[index] ? `p. ${props.pages[index]}` : '')
</script>

<template>
  <div ref="box" class="source" role="document" aria-label="PBN source">
    <template v-for="(run, i) in runs" :key="i">
      <pre
        v-if="run.index < 0"
        class="gap"
        v-html="run.html"
      ></pre>
      <pre
        v-else
        class="record"
        :class="{ on: run.index === highlighted }"
        :data-record="run.index"
        :data-page="pageLabel(run.index)"
        :title="pages[run.index] ? `Board ${run.index + 1} — page ${pages[run.index]}` : ''"
        @click="emit('select', run.index)"
        v-html="run.html"
      ></pre>
    </template>
  </div>
</template>

<style scoped>
.source {
  height: min(72vh, 900px); overflow: auto; overscroll-behavior: contain;
  border: 1px solid var(--line); border-radius: var(--radius);
  background: var(--panel); padding: 0.4rem 0;
  font: 12px/1.45 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
pre { margin: 0; padding: 0 0.6rem; white-space: pre-wrap; word-break: break-word; font: inherit; }
.record { position: relative; cursor: pointer; border-left: 3px solid transparent; }
.record:hover { background: color-mix(in srgb, var(--accent) 6%, transparent); }
.record.on {
  background: color-mix(in srgb, #e0b000 20%, transparent);
  border-left-color: #c79a00;
}
.record[data-page]:not([data-page=''])::after {
  content: attr(data-page); position: absolute; top: 0; right: 0.5rem;
  color: var(--muted); font-size: 0.85em;
}
.source :deep(.t) { color: #1d5fbf; font-weight: 600; }
.source :deep(.v) { color: #8a5a1c; }
.source :deep(.d) { color: var(--muted); }
.source :deep(.m) { color: var(--muted); font-style: italic; }
.source :deep(.c) { color: #2b7a3d; }
.source :deep(.r) { color: #c0392b; }
.source :deep(.a) { color: #c46a00; }
@media (prefers-color-scheme: dark) {
  .source :deep(.t) { color: #79a8f0; }
  .source :deep(.v) { color: #d6a466; }
  .source :deep(.c) { color: #6cc584; }
  .source :deep(.r) { color: #ef7b6e; }
  .source :deep(.a) { color: #f0a050; }
}
</style>
