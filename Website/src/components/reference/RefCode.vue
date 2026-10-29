<script setup lang="ts">
import { computed, ref } from 'vue'
import { highlight } from '@/utils/highlight'

const props = withDefaults(
  defineProps<{ code: string; lang?: string; label?: string; wrap?: boolean }>(),
  { lang: 'swift', label: '', wrap: false },
)

const copied = ref(false)
const html = computed(() => highlight(props.code, props.lang))

async function copy() {
  try {
    await navigator.clipboard.writeText(props.code)
    copied.value = true
    setTimeout(() => (copied.value = false), 1500)
  } catch {
    /* clipboard unavailable — leave the button idle */
  }
}
</script>

<template>
  <figure class="ref-code" :class="{ wrap }">
    <figcaption>
      <span>{{ label || lang }}</span>
      <button type="button" @click="copy">{{ copied ? '已复制' : '复制' }}</button>
    </figcaption>
    <pre><code v-html="html"></code></pre>
  </figure>
</template>

<style scoped>
.ref-code {
  margin: 0;
  min-width: 0;
  max-width: 100%;
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
  background: #030d18;
}

figcaption {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 7px 14px;
  border-bottom: 1px solid var(--border);
  background: rgba(12, 46, 68, 0.5);
  font-family: var(--font-mono);
  font-size: 11.5px;
  letter-spacing: 0.04em;
  color: var(--text-faint);
}

button {
  border: none;
  background: none;
  color: var(--text-dim);
  font-family: var(--font-sans);
  font-size: 12px;
  padding: 3px 9px;
  border-radius: 6px;
  cursor: pointer;
}

button:hover {
  color: var(--primary);
  background: rgba(34, 211, 238, 0.1);
}

pre {
  margin: 0;
  padding: 16px 18px;
  overflow-x: auto;
}

.ref-code.wrap code {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

code {
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.75;
  color: #cbe8f3;
  white-space: pre;
}

code :deep(.tok-keyword) {
  color: #67e8f9;
  font-weight: 600;
}
code :deep(.tok-string) {
  color: #86efac;
}
code :deep(.tok-comment) {
  color: #5d8294;
  font-style: italic;
}
code :deep(.tok-number) {
  color: #fbbf24;
}
</style>
