<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import RefCode from '@/components/reference/RefCode.vue'
import { extractToc, renderMarkdown } from '@/utils/markdown'
import {
  allSymbols,
  availability,
  categories,
  categoryBlurbs,
  categoryById,
  categoryTitle,
  findSymbol,
  framework,
  groupMembers,
  inline,
  kindLabel,
  navGroups,
  overviewFor,
  searchSymbols,
  symbolsInCategory,
  type ApiSymbol,
  type IndexEntry,
} from '@/utils/reference'

const route = useRoute()

const categoryId = computed(() => (route.params.category as string) ?? '')
const symbolId = computed(() => (route.params.symbol as string) ?? '')

const categorySymbols = ref<ApiSymbol[]>([])
const currentSymbol = ref<ApiSymbol | null>(null)
const loading = ref(false)
const notFound = ref(false)

const query = ref('')
const searchResults = computed<IndexEntry[]>(() => searchSymbols(query.value))
const navOpen = ref(false)

const mainEl = ref<HTMLElement | null>(null)
const activeHeading = ref('')

const byName = computed(() => {
  const map = new Map<string, IndexEntry>()
  for (const s of allSymbols) if (!map.has(s.name)) map.set(s.name, s)
  return map
})

const popularIds: { category: string; id: string }[] = [
  { category: 'core', id: 'view' },
  { category: 'core', id: 'box' },
  { category: 'core', id: 'row' },
  { category: 'core', id: 'column' },
  { category: 'core', id: 'state' },
  { category: 'text', id: 'text' },
  { category: 'controls', id: 'button' },
  { category: 'layout', id: 'scroll-view' },
  { category: 'layout', id: 'tab-view' },
  { category: 'data', id: 'list' },
  { category: 'theme', id: 'theme' },
  { category: 'app', id: 'application' },
]

const popular = computed(() =>
  popularIds
    .map(({ category, id }) => {
      const entry = allSymbols.find((s) => s.id === id && s.category === category)
      return entry ? { ...entry, category } : null
    })
    .filter((s): s is IndexEntry & { category: string } => s !== null),
)

const categoryCards = computed(() =>
  navGroups.flatMap((g) =>
    g.categories
      .map((id) => categories.find((c) => c.id === id))
      .filter((c): c is (typeof categories)[number] => Boolean(c))
      .map((c) => ({ ...c, blurb: categoryBlurbs[c.id] ?? '' })),
  ),
)

const currentCategory = computed(() =>
  categoryId.value ? categories.find((c) => c.id === categoryId.value) : undefined,
)

const isFramework = computed(() => !categoryId.value)
const isCategoryPage = computed(() => Boolean(categoryId.value) && !symbolId.value)

const frameworkHtml = computed(() => renderMarkdown(framework))
const frameworkToc = computed(() => extractToc(frameworkHtml.value))

const overviewHtml = computed(() =>
  currentSymbol.value ? renderMarkdown(overviewFor(currentSymbol.value.id)) : '',
)

const discussionHtml = computed(() => {
  const symbol = currentSymbol.value
  if (!symbol) return ''
  return symbol.discussion.map((p) => renderMarkdown(p)).join('')
})

const memberGroups = computed(() => (currentSymbol.value ? groupMembers(currentSymbol.value.members) : []))

const categoryGroups = computed(() => {
  const types = categorySymbols.value.filter((s) =>
    ['struct', 'enum', 'protocol', 'class', 'actor'].includes(s.kind),
  )
  const modifiers = categorySymbols.value.filter((s) => s.kind === 'modifier')
  const others = categorySymbols.value.filter(
    (s) => !types.includes(s) && s.kind !== 'modifier',
  )
  return [
    { title: '类型', items: types },
    { title: '视图修饰符', items: modifiers },
    { title: '函数与属性', items: others },
  ].filter((g) => g.items.length > 0)
})

const conformLinks = computed(() =>
  (currentSymbol.value?.conforms ?? []).map((name) => ({ name, href: resolve(name) })),
)

const conformedBy = computed(() => {
  const symbol = currentSymbol.value
  if (!symbol) return []
  return allSymbols
    .filter(
      (s) =>
        s.conforms?.includes(symbol.name) &&
        !s.name.startsWith('_') &&
        !(s.id === symbol.id && s.category === symbol.category),
    )
    .sort((a, b) => a.name.localeCompare(b.name))
})

const siblings = computed(() => {
  const symbol = currentSymbol.value
  if (!symbol) return []
  return categorySymbols.value
    .filter((s) => s.id !== symbol.id)
    .sort((a, b) => {
      const sameA = a.kind === symbol.kind ? 0 : 1
      const sameB = b.kind === symbol.kind ? 0 : 1
      return sameA - sameB || a.name.localeCompare(b.name)
    })
    .slice(0, 6)
})

const toc = computed(() => {
  if (isFramework.value) {
    return [
      { id: 'overview', text: '概述', depth: 2 },
      ...frameworkToc.value.map((h) => ({ id: h.id, text: h.text, depth: 2 })),
      { id: 'explore', text: '探索', depth: 2 },
      { id: 'popular', text: '常用符号', depth: 2 },
    ]
  }
  if (isCategoryPage.value) {
    return categoryGroups.value.map((g) => ({
      id: `topic-${slug(g.title)}`,
      text: g.title,
      depth: 2,
    }))
  }
  const items = [
    { id: 'declaration', text: '声明', depth: 2 },
    { id: 'overview', text: '概述', depth: 2 },
  ]
  if (memberGroups.value.length) items.push({ id: 'topics', text: '主题', depth: 2 })
  if (conformLinks.value.length || conformedBy.value.length) {
    items.push({ id: 'relationships', text: '关系', depth: 2 })
  }
  items.push({ id: 'see-also', text: '另请参阅', depth: 2 })
  items.push({ id: 'source', text: '源码', depth: 2 })
  return items
})

function slug(text: string): string {
  return text.toLowerCase().replace(/\s+/g, '-')
}

function symbolLink(cat: string, id: string) {
  return `/documentation/coveui/${cat}/${id}`
}

function resolve(name: string) {
  const entry = byName.value.get(name)
  return entry ? symbolLink(entry.category, entry.id) : null
}

async function load() {
  navOpen.value = false
  notFound.value = false
  if (!categoryId.value) {
    categorySymbols.value = []
    currentSymbol.value = null
    setTitle('CoveUI 参考')
    return
  }
  if (!categoryById(categoryId.value)) {
    categorySymbols.value = []
    currentSymbol.value = null
    notFound.value = true
    setTitle('未找到分类 · CoveUI')
    return
  }
  loading.value = true
  const list = await symbolsInCategory(categoryId.value)
  categorySymbols.value = list
  if (symbolId.value) {
    const found = await findSymbol(categoryId.value, symbolId.value)
    currentSymbol.value = found ?? null
    notFound.value = !found
    setTitle(found ? `${found.name} · CoveUI` : '未找到符号 · CoveUI')
  } else {
    currentSymbol.value = null
    setTitle(`${categoryTitle(categoryId.value)} · CoveUI`)
  }
  loading.value = false
  await nextTick()
  window.scrollTo({ top: 0 })
  setupSpy()
}

function setTitle(title: string) {
  document.title = title
}

function onContentClick(e: MouseEvent) {
  const btn = (e.target as HTMLElement).closest('.md-copy') as HTMLButtonElement | null
  if (!btn) return
  const code = btn.closest('.md-code')?.getAttribute('data-code') ?? ''
  navigator.clipboard.writeText(code).then(() => {
    btn.textContent = '已复制'
    setTimeout(() => (btn.textContent = '复制'), 1500)
  })
}

let observer: IntersectionObserver | null = null

function setupSpy() {
  observer?.disconnect()
  const headings = Array.from(
    mainEl.value?.querySelectorAll<HTMLElement>('[data-spy], h2[id]') ?? [],
  )
  if (!headings.length || !('IntersectionObserver' in window)) return
  observer = new IntersectionObserver(
    (entries) => {
      const visible = entries.filter((e) => e.isIntersecting)
      if (visible.length) activeHeading.value = visible[0].target.id
    },
    { rootMargin: '-96px 0px -68% 0px', threshold: 0 },
  )
  headings.forEach((h) => observer?.observe(h))
}

watch(() => route.fullPath, load, { immediate: true })
onMounted(() => nextTick(setupSpy))
onBeforeUnmount(() => observer?.disconnect())
</script>

<template>
  <div class="ref">
    <div class="ref-scrim" :class="{ open: navOpen }" @click="navOpen = false"></div>

    <aside class="ref-side" :class="{ open: navOpen }">
      <div class="side-head">
        <RouterLink to="/documentation/coveui" class="fw-link">
          <span class="fw-mark">C</span>
          <span>
            <strong>CoveUI</strong>
            <em>框架参考</em>
          </span>
        </RouterLink>
        <label class="search">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-3.5-3.5" stroke-linecap="round" />
          </svg>
          <input v-model="query" type="search" placeholder="搜索符号" aria-label="搜索符号" />
        </label>
      </div>

      <nav class="side-body">
        <template v-if="query.trim()">
          <p class="side-label">搜索结果 · {{ searchResults.length }}</p>
          <RouterLink
            v-for="r in searchResults"
            :key="`${r.category}/${r.id}`"
            :to="symbolLink(r.category, r.id)"
            class="result"
          >
            <span class="result-name">{{ r.name }}</span>
            <span class="result-meta">{{ kindLabel(r.kind) }} · {{ categoryTitle(r.category) }}</span>
          </RouterLink>
          <p v-if="!searchResults.length" class="side-empty">没有匹配的符号。</p>
        </template>

        <template v-else>
          <RouterLink
            to="/documentation/coveui"
            class="side-link framework"
            :class="{ active: isFramework }"
          >
            CoveUI 概览
          </RouterLink>
          <div v-for="g in navGroups" :key="g.title" class="side-group">
            <p class="side-label">{{ g.title }}</p>
            <template v-for="cid in g.categories" :key="cid">
              <RouterLink
                v-if="categories.find((c) => c.id === cid)"
                :to="`/documentation/coveui/${cid}`"
                class="side-link"
                :class="{ active: categoryId === cid && !symbolId }"
              >
                {{ categoryTitle(cid) }}
                <span class="count">{{ categories.find((c) => c.id === cid)?.count }}</span>
              </RouterLink>
            </template>
          </div>
          <div class="side-group">
            <p class="side-label">指南</p>
            <RouterLink to="/docs" class="side-link">快速开始与概念</RouterLink>
          </div>
        </template>
      </nav>
    </aside>

    <main ref="mainEl" class="ref-main" @click="onContentClick">
      <button class="nav-toggle" type="button" @click="navOpen = true">目录</button>

      <nav class="crumbs" aria-label="面包屑">
        <RouterLink to="/documentation/coveui">CoveUI</RouterLink>
        <template v-if="categoryId">
          <span>/</span>
          <RouterLink :to="`/documentation/coveui/${categoryId}`">{{ categoryTitle(categoryId) }}</RouterLink>
        </template>
        <template v-if="symbolId && currentSymbol">
          <span>/</span>
          <span class="crumb-current">{{ currentSymbol.name }}</span>
        </template>
      </nav>

      <!-- Framework landing -->
      <template v-if="isFramework">
        <header class="page-head">
          <p class="kind">框架</p>
          <h1>CoveUI</h1>
          <p class="lead">
            Swift 跨平台 UI 框架。声明式视图、统一布局与文本图像管线，通过 NativeRHI 驱动
            Metal、DirectX 12 与 Vulkan。
          </p>
          <div class="chips">
            <span v-for="p in availability" :key="p" class="chip">{{ p }}</span>
            <span class="chip muted">Windows x86_64</span>
            <span class="chip muted">Linux x86_64</span>
          </div>
          <div class="head-actions">
            <RouterLink to="/docs" class="action primary">阅读指南</RouterLink>
            <a class="action" href="https://github.com/AlexCat315/Cove" target="_blank" rel="noopener">
              源码仓库
            </a>
          </div>
        </header>

        <section id="overview" data-spy class="ref-section markdown-body" v-html="frameworkHtml"></section>

        <section id="explore" data-spy class="ref-section">
          <h2>探索</h2>
          <div class="cards">
            <RouterLink
              v-for="c in categoryCards"
              :key="c.id"
              :to="`/documentation/coveui/${c.id}`"
              class="card"
            >
              <span class="card-top">
                <strong>{{ c.title }}</strong>
                <em>{{ c.count }}</em>
              </span>
              <span class="card-blurb">{{ c.blurb }}</span>
            </RouterLink>
          </div>
        </section>

        <section id="popular" data-spy class="ref-section">
          <h2>常用符号</h2>
          <ul class="rows">
            <li v-for="s in popular" :key="`${s.category}/${s.id}`">
              <RouterLink :to="symbolLink(s.category, s.id)">
                <span class="row-name">{{ s.name }}</span>
                <span class="row-abstract">{{ s.abstract }}</span>
                <span class="chev">›</span>
              </RouterLink>
            </li>
          </ul>
        </section>
      </template>

      <!-- Category -->
      <template v-else-if="isCategoryPage && currentCategory">
        <header class="page-head">
          <p class="kind">分类</p>
          <h1>{{ currentCategory?.title }}</h1>
          <p class="lead">{{ categoryBlurbs[categoryId] }}</p>
          <div class="chips">
            <span class="chip">{{ categorySymbols.length }} 个符号</span>
            <span class="chip muted">{{ categoryId }}</span>
          </div>
        </header>

        <section
          v-for="g in categoryGroups"
          :id="`topic-${slug(g.title)}`"
          :key="g.title"
          data-spy
          class="ref-section"
        >
          <h2>{{ g.title }}</h2>
          <ul class="rows">
            <li v-for="s in g.items" :key="s.id">
              <RouterLink :to="symbolLink(categoryId, s.id)">
                <span class="row-name">{{ s.name }}</span>
                <span class="row-abstract" v-html="inline(s.abstract || s.declaration)"></span>
                <span class="chev">›</span>
              </RouterLink>
            </li>
          </ul>
        </section>
      </template>

      <!-- Symbol -->
      <template v-else-if="currentSymbol">
        <header class="page-head">
          <p class="kind">{{ kindLabel(currentSymbol.kind) }}</p>
          <h1>{{ currentSymbol.name }}</h1>
          <p v-if="currentSymbol.abstract" class="lead" v-html="inline(currentSymbol.abstract)"></p>
          <div class="chips">
            <span v-for="p in availability" :key="p" class="chip">{{ p }}</span>
          </div>
        </header>

        <section id="declaration" data-spy class="ref-section">
          <h2>声明</h2>
          <RefCode
            v-for="(decl, i) in currentSymbol.overloads"
            :key="i"
            :code="decl"
            label="Swift"
            class="decl"
            wrap
          />
        </section>

        <section id="overview" data-spy class="ref-section">
          <h2>概述</h2>
          <div v-if="overviewHtml" class="markdown-body" v-html="overviewHtml"></div>
          <div v-else-if="discussionHtml" class="markdown-body" v-html="discussionHtml"></div>
          <p v-else class="empty-note">
            源码中的这个符号没有附加文档注释。声明与成员列表直接取自
            <code>{{ currentSymbol.file }}</code>。
          </p>
        </section>

        <section v-if="memberGroups.length" id="topics" data-spy class="ref-section">
          <h2>主题</h2>
          <div v-for="g in memberGroups" :key="g.title" class="member-group">
            <h3>{{ g.title }}</h3>
            <ul class="members">
              <li v-for="(m, i) in g.items" :key="`${m.name}-${i}`">
                <code class="member-decl">{{ m.declaration }}</code>
                <p v-if="m.abstract" class="member-abstract" v-html="inline(m.abstract)"></p>
              </li>
            </ul>
          </div>
        </section>

        <section
          v-if="conformLinks.length || conformedBy.length"
          id="relationships"
          data-spy
          class="ref-section"
        >
          <h2>关系</h2>
          <dl v-if="conformLinks.length" class="relations">
            <dt>遵循</dt>
            <dd>
              <template v-for="(c, i) in conformLinks" :key="c.name">
                <RouterLink v-if="c.href" :to="c.href">{{ c.name }}</RouterLink>
                <code v-else>{{ c.name }}</code>
                <span v-if="i < conformLinks.length - 1">, </span>
              </template>
            </dd>
          </dl>
          <dl v-if="conformedBy.length" class="relations">
            <dt>被遵循</dt>
            <dd>
              <template v-for="(c, i) in conformedBy.slice(0, 16)" :key="`${c.category}/${c.id}`">
                <RouterLink :to="symbolLink(c.category, c.id)">{{ c.name }}</RouterLink>
                <span v-if="i < Math.min(conformedBy.length, 16) - 1">, </span>
              </template>
              <span v-if="conformedBy.length > 16" class="muted">
                等 {{ conformedBy.length }} 个类型
              </span>
            </dd>
          </dl>
        </section>

        <section id="see-also" data-spy class="ref-section">
          <h2>另请参阅</h2>
          <ul class="rows">
            <li v-for="s in siblings" :key="s.id">
              <RouterLink :to="symbolLink(categoryId, s.id)">
                <span class="row-name">{{ s.name }}</span>
                <span class="row-abstract" v-html="inline(s.abstract || s.declaration)"></span>
                <span class="chev">›</span>
              </RouterLink>
            </li>
          </ul>
        </section>

        <section id="source" data-spy class="section source">
          <h2>源码</h2>
          <p>
            <code>{{ currentSymbol.file }}</code>
            <span class="line">:{{ currentSymbol.line }}</span>
          </p>
          <p class="muted">模块 {{ currentSymbol.module }} · 分类 {{ categoryTitle(currentSymbol.category) }}</p>
        </section>
      </template>

      <template v-else-if="notFound">
        <header class="page-head">
          <p class="kind">未找到</p>
          <h1>没有这个符号</h1>
          <p class="lead">该页面可能已随源码变动被移除或重命名。</p>
          <p><RouterLink class="action" :to="`/documentation/coveui/${categoryId}`">返回 {{ categoryTitle(categoryId) }}</RouterLink></p>
        </header>
      </template>

      <p v-if="loading" class="loading">正在读取参考数据…</p>
    </main>

    <aside class="ref-toc">
      <p class="toc-title">本页内容</p>
      <a
        v-for="h in toc"
        :key="h.id"
        :href="`#${h.id}`"
        class="toc-link"
        :class="{ active: activeHeading === h.id }"
      >
        {{ h.text }}
      </a>
    </aside>
  </div>
</template>

<style scoped>
.ref {
  display: grid;
  grid-template-columns: 268px minmax(0, 1fr) 220px;
  max-width: 1440px;
  margin: 0 auto;
  padding-top: var(--nav-h);
  min-height: 100vh;
}

/* ---------- Sidebar ---------- */
.ref-side {
  position: sticky;
  top: var(--nav-h);
  height: calc(100vh - var(--nav-h));
  overflow-y: auto;
  border-right: 1px solid var(--border);
  padding: 26px 14px 40px;
}

.side-head {
  padding: 0 8px 16px;
  border-bottom: 1px solid var(--border);
}

.fw-link {
  display: flex;
  gap: 10px;
  align-items: center;
  margin-bottom: 16px;
}

.fw-mark {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  display: grid;
  place-items: center;
  font-weight: 800;
  color: #03262c;
  background: var(--gradient);
}

.fw-link strong {
  display: block;
  font-size: 15px;
  line-height: 1.2;
}

.fw-link em {
  font-style: normal;
  font-size: 11.5px;
  color: var(--text-faint);
}

.search {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 7px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-faint);
}

.search input {
  flex: 1;
  min-width: 0;
  border: none;
  background: none;
  outline: none;
  color: var(--text);
  font-family: var(--font-sans);
  font-size: 13px;
}

.search input::placeholder {
  color: var(--text-faint);
}

.side-body {
  padding-top: 8px;
}

.side-label {
  font-size: 11px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--text-faint);
  padding: 16px 10px 6px;
}

.side-link {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 7px;
  font-size: 13.5px;
  color: var(--text-dim);
  border-left: 2px solid transparent;
}

.side-link:hover {
  color: var(--text);
  background: rgba(255, 255, 255, 0.04);
}

.side-link.active {
  color: var(--primary);
  background: rgba(34, 211, 238, 0.08);
  border-left-color: var(--primary);
}

.side-link.framework {
  font-weight: 600;
  color: var(--text);
}

.side-link .count {
  font-size: 11px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.result {
  display: block;
  padding: 8px 10px;
  border-radius: 7px;
}

.result:hover {
  background: rgba(255, 255, 255, 0.05);
}

.result-name {
  display: block;
  font-size: 13.5px;
  color: var(--text);
}

.result-meta {
  display: block;
  font-size: 11.5px;
  color: var(--text-faint);
}

.side-empty {
  padding: 12px 10px;
  font-size: 13px;
  color: var(--text-faint);
}

/* ---------- Main ---------- */
.ref-main {
  padding: 34px 54px 96px;
  min-width: 0;
  max-width: 900px;
}

.nav-toggle {
  display: none;
}

.crumbs {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 12.5px;
  color: var(--text-faint);
  margin-bottom: 26px;
}

.crumbs a:hover {
  color: var(--primary);
}

.crumb-current {
  color: var(--text-dim);
}

.page-head {
  padding-bottom: 26px;
  border-bottom: 1px solid var(--border);
}

.kind {
  font-size: 12px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--primary);
  margin-bottom: 10px;
}

.page-head h1 {
  font-size: clamp(32px, 4.4vw, 44px);
  letter-spacing: -0.035em;
  margin-bottom: 12px;
}

.lead {
  font-size: 17px;
  color: var(--text-dim);
  max-width: 64ch;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 18px;
}

.chip {
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 6px;
  border: 1px solid var(--border-strong);
  color: var(--text-dim);
  background: rgba(34, 211, 238, 0.06);
}

.chip.muted {
  border-color: var(--border);
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-faint);
}

.head-actions {
  display: flex;
  gap: 10px;
  margin-top: 22px;
}

.action {
  display: inline-flex;
  align-items: center;
  padding: 9px 18px;
  border-radius: 999px;
  border: 1px solid var(--border-strong);
  font-size: 13.5px;
  color: var(--text);
}

.action:hover {
  border-color: var(--primary);
  background: rgba(34, 211, 238, 0.08);
}

.action.primary {
  background: var(--gradient);
  color: #03262c;
  border-color: transparent;
  font-weight: 600;
}

.ref-section {
  margin-top: 40px;
  scroll-margin-top: calc(var(--nav-h) + 24px);
}

.ref-section h2 {
  font-size: 24px;
  margin-bottom: 16px;
}

.ref-section h3 {
  font-size: 15px;
  color: var(--text-dim);
  letter-spacing: 0.02em;
  margin: 26px 0 10px;
}

/* ---------- Cards ---------- */
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
  gap: 12px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface);
  transition: border-color 0.2s, transform 0.2s;
}

.card:hover {
  border-color: var(--primary);
  transform: translateY(-2px);
}

.card-top {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}

.card-top strong {
  font-size: 15.5px;
}

.card-top em {
  font-style: normal;
  font-size: 11.5px;
  color: var(--text-faint);
  font-variant-numeric: tabular-nums;
}

.card-blurb {
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-dim);
}

/* ---------- Rows ---------- */
.rows {
  list-style: none;
  border-top: 1px solid var(--border);
}

.rows li {
  border-bottom: 1px solid var(--border);
}

.rows a {
  display: grid;
  grid-template-columns: minmax(120px, 210px) 1fr 14px;
  gap: 14px;
  align-items: baseline;
  padding: 12px 4px;
}

.rows a:hover .row-name {
  color: var(--primary);
}

.row-name {
  font-size: 14.5px;
  font-weight: 600;
  color: var(--text);
  font-family: var(--font-mono);
  letter-spacing: -0.01em;
}

.row-abstract {
  font-size: 13px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chev {
  color: var(--text-faint);
  justify-self: end;
}

/* ---------- Members ---------- */
.members {
  list-style: none;
  border-top: 1px solid var(--border);
}

.members li {
  padding: 12px 2px;
  border-bottom: 1px solid var(--border);
}

.member-decl {
  min-width: 0;
  word-break: break-word;
  font-size: 13px;
  color: var(--primary-soft);
  background: none;
  padding: 0;
}

.member-abstract {
  font-size: 12.5px;
  color: var(--text-dim);
  margin-top: 5px;
}

.relations {
  display: flex;
  gap: 10px;
  font-size: 14px;
  align-items: baseline;
}

.relations + .relations {
  margin-top: 10px;
}

.relations dt {
  flex: 0 0 52px;
  color: var(--text-faint);
}

.relations dd {
  line-height: 1.9;
}

.relations a {
  color: var(--primary);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.empty-note {
  font-size: 13.5px;
  color: var(--text-faint);
  border-left: 2px solid var(--border-strong);
  padding-left: 14px;
}

.source p {
  font-size: 13.5px;
}

.source code {
  overflow-wrap: anywhere;
  color: var(--primary-soft);
  background: rgba(34, 211, 238, 0.08);
  padding: 2px 7px;
  border-radius: 6px;
}

.source .line {
  color: var(--text-faint);
}

.muted {
  color: var(--text-faint);
  font-size: 12.5px;
  margin-top: 6px;
}

.loading {
  color: var(--text-faint);
  font-size: 14px;
  margin-top: 30px;
}

/* ---------- TOC ---------- */
.ref-toc {
  position: sticky;
  top: var(--nav-h);
  height: calc(100vh - var(--nav-h));
  overflow-y: auto;
  padding: 40px 18px;
  border-left: 1px solid var(--border);
}

.toc-title {
  font-size: 11px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--text-faint);
  margin-bottom: 12px;
}

.toc-link {
  display: block;
  font-size: 12.5px;
  color: var(--text-faint);
  padding: 5px 0 5px 11px;
  border-left: 2px solid transparent;
}

.toc-link:hover {
  color: var(--text-dim);
}

.toc-link.active {
  color: var(--primary);
  border-left-color: var(--primary);
}

.ref-scrim {
  display: none;
}

/* ---------- Markdown (shared with guides) ---------- */
.markdown-body :deep(h2) {
  font-size: 24px;
  margin: 34px 0 14px;
}

.markdown-body :deep(h3) {
  font-size: 18px;
  margin: 26px 0 10px;
}

.markdown-body :deep(p) {
  margin: 12px 0;
  color: var(--text);
  font-size: 15px;
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
  margin: 12px 0;
  padding-left: 24px;
  font-size: 15px;
}

.markdown-body :deep(li) {
  margin: 6px 0;
}

.markdown-body :deep(a) {
  color: var(--primary);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.markdown-body :deep(code) {
  font-family: var(--font-mono);
  font-size: 13px;
  background: rgba(34, 211, 238, 0.1);
  color: var(--primary-soft);
  padding: 2px 6px;
  border-radius: 5px;
}

.markdown-body :deep(table) {
  width: 100%;
  border-collapse: collapse;
  margin: 18px 0;
  font-size: 13.5px;
}

.markdown-body :deep(th),
.markdown-body :deep(td) {
  border: 1px solid var(--border);
  padding: 9px 12px;
  text-align: left;
}

.markdown-body :deep(th) {
  background: rgba(12, 46, 68, 0.5);
}

.markdown-body :deep(.md-code) {
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
  margin: 18px 0;
  background: #030d18;
}

.markdown-body :deep(.md-code-head) {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 7px 14px;
  background: rgba(12, 46, 68, 0.5);
  border-bottom: 1px solid var(--border);
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-faint);
}

.markdown-body :deep(.md-copy) {
  border: none;
  background: none;
  color: var(--text-dim);
  font-size: 12px;
  cursor: pointer;
  padding: 3px 9px;
  border-radius: 6px;
  font-family: var(--font-sans);
}

.markdown-body :deep(.md-copy:hover) {
  color: var(--primary);
  background: rgba(34, 211, 238, 0.1);
}

.markdown-body :deep(.md-code pre) {
  margin: 0;
  padding: 16px 18px;
  overflow-x: auto;
}

.markdown-body :deep(.md-code pre code) {
  background: none;
  color: #cbe8f3;
  padding: 0;
  font-size: 13px;
  line-height: 1.75;
}

.markdown-body :deep(.md-code .tok-keyword) {
  color: #67e8f9;
  font-weight: 600;
}
.markdown-body :deep(.md-code .tok-string) {
  color: #86efac;
}
.markdown-body :deep(.md-code .tok-comment) {
  color: #5d8294;
  font-style: italic;
}
.markdown-body :deep(.md-code .tok-number) {
  color: #fbbf24;
}

.decl + .decl {
  margin-top: 10px;
}

/* ---------- Responsive ---------- */
@media (max-width: 1180px) {
  .ref {
    grid-template-columns: 250px minmax(0, 1fr);
  }
  .ref-toc {
    display: none;
  }
  .ref-main {
    padding: 30px 38px 80px;
  }
}

@media (max-width: 880px) {
  .ref {
    grid-template-columns: minmax(0, 1fr);
  }
  .ref-side {
    position: fixed;
    top: var(--nav-h);
    left: 0;
    bottom: 0;
    z-index: 95;
    width: 286px;
    background: rgba(4, 18, 31, 0.98);
    backdrop-filter: blur(18px);
    transform: translateX(-100%);
    transition: transform 0.28s ease;
  }
  .ref-side.open {
    transform: none;
  }
  .ref-scrim {
    display: block;
    position: fixed;
    inset: var(--nav-h) 0 0 0;
    z-index: 90;
    background: rgba(0, 0, 0, 0.55);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.28s;
  }
  .ref-scrim.open {
    opacity: 1;
    pointer-events: auto;
  }
  .nav-toggle {
    display: inline-flex;
    margin-bottom: 18px;
    padding: 8px 14px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text-dim);
    font-size: 13px;
    cursor: pointer;
  }
  .ref-main {
    padding: 22px 20px 70px;
  }
  .rows a {
    grid-template-columns: 1fr 14px;
  }
  .row-abstract {
    grid-column: 1 / -1;
    white-space: normal;
  }
}
</style>
