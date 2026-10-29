/**
 * Reference data access. The symbol index and every category chunk are
 * generated from the Swift sources by `scripts/extract-api.py`; nothing on
 * these pages is invented here.
 */
import { marked } from 'marked'
import indexJson from '@/content/api/index.json'
import frameworkRaw from '@/content/reference/framework.md?raw'
import { categoryBlurbs, navGroups } from '@/content/reference/categories'

export type SymbolKind =
  | 'struct'
  | 'enum'
  | 'protocol'
  | 'class'
  | 'actor'
  | 'func'
  | 'var'
  | 'let'
  | 'init'
  | 'subscript'
  | 'modifier'

export interface ApiMember {
  name: string
  kind: string
  declaration: string
  abstract: string
  discussion: string[]
}

export interface ApiSymbol {
  id: string
  name: string
  kind: SymbolKind
  declaration: string
  overloads: string[]
  abstract: string
  discussion: string[]
  availability: string[]
  conforms: string[]
  members: ApiMember[]
  module: string
  category: string
  file: string
  line: number
  owner: string
}

export interface ApiCategory {
  id: string
  title: string
  count: number
}

export interface IndexEntry {
  id: string
  name: string
  kind: SymbolKind
  category: string
  abstract: string
  conforms: string[]
  module: string
}

const index = indexJson as { categories: ApiCategory[]; symbols: IndexEntry[] }

export const categories = index.categories
export const allSymbols = index.symbols
export const framework = frameworkRaw
export { navGroups, categoryBlurbs }

const chunks = import.meta.glob('../content/api/*.json')
const symbolDocs = import.meta.glob('../content/reference/symbols/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const loaded = new Map<string, ApiSymbol[]>()

export function categoryById(id: string): ApiCategory | undefined {
  return categories.find((c) => c.id === id)
}

export function categoryTitle(id: string): string {
  return categoryById(id)?.title ?? id
}

export async function symbolsInCategory(id: string): Promise<ApiSymbol[]> {
  if (loaded.has(id)) return loaded.get(id)!
  const key = `../content/api/${id}.json`
  const loader = chunks[key]
  if (!loader) return []
  const mod = (await loader()) as { default?: ApiSymbol[] } | ApiSymbol[]
  const list = Array.isArray(mod) ? mod : (mod.default ?? [])
  loaded.set(id, list)
  return list
}

export async function findSymbol(category: string, id: string): Promise<ApiSymbol | undefined> {
  const list = await symbolsInCategory(category)
  return list.find((s) => s.id === id)
}

/** Curated overview markdown, when one has been written for this symbol. */
export function overviewFor(id: string): string {
  const key = `../content/reference/symbols/${id}.md`
  return symbolDocs[key] ?? ''
}

/** Inline markdown for abstracts (backticked symbols, emphasis). */
export function inline(text: string): string {
  if (!text) return ''
  return marked.parseInline(text) as string
}

export function kindLabel(kind: SymbolKind): string {
  switch (kind) {
    case 'struct':
      return '结构体'
    case 'enum':
      return '枚举'
    case 'protocol':
      return '协议'
    case 'class':
      return '类'
    case 'actor':
      return 'Actor'
    case 'modifier':
      return '视图修饰符'
    case 'init':
      return '初始化器'
    case 'func':
      return '函数'
    default:
      return '属性'
  }
}

/** Kind buckets used by the "主题" sections on symbol and category pages. */
export const memberGroups: { title: string; kinds: string[] }[] = [
  { title: '初始化器', kinds: ['init'] },
  { title: '实例属性', kinds: ['let', 'var'] },
  { title: '实例方法', kinds: ['func', 'subscript'] },
]

export function groupMembers(members: ApiMember[]) {
  return memberGroups
    .map((g) => ({ title: g.title, items: members.filter((m) => g.kinds.includes(m.kind)) }))
    .filter((g) => g.items.length > 0)
}

export function searchSymbols(query: string, limit = 30): IndexEntry[] {
  const q = query.trim().toLowerCase()
  if (!q) return []
  const scored: { entry: IndexEntry; score: number }[] = []
  for (const entry of allSymbols) {
    const name = entry.name.toLowerCase()
    const abstract = entry.abstract.toLowerCase()
    let score = 0
    if (name === q) score = 100
    else if (name.startsWith(q)) score = 80
    else if (name.includes(q)) score = 60
    else if (abstract.includes(q)) score = 30
    if (score) scored.push({ entry, score })
  }
  return scored
    .sort((a, b) => b.score - a.score || a.entry.name.length - b.entry.name.length)
    .slice(0, limit)
    .map((s) => s.entry)
}

/** Platforms come from Package.swift (`macOS(.v26)`, `iOS(.v26)`) and the SDK. */
export const availability = ['macOS 26.0+', 'iOS 26.0+']
