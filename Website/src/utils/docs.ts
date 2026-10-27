/**
 * Documentation manifest — the sidebar tree. Each leaf maps a slug
 * to a raw markdown file imported via Vite's `?raw` suffix.
 */
export interface DocLeaf {
  slug: string
  title: string
  raw: string
}
export interface DocGroup {
  group: string
  items: DocLeaf[]
}

import gettingStarted from '@/content/getting-started.md?raw'
import architecture from '@/content/architecture.md?raw'
import views from '@/content/views.md?raw'
import nativeRhi from '@/content/native-rhi.md?raw'
import cli from '@/content/cli.md?raw'

export const docGroups: DocGroup[] = [
  {
    group: '指南',
    items: [
      { slug: 'getting-started', title: '快速开始', raw: gettingStarted },
      { slug: 'architecture', title: '架构概览', raw: architecture },
    ],
  },
  {
    group: '核心概念',
    items: [
      { slug: 'views', title: '视图与布局', raw: views },
      { slug: 'native-rhi', title: 'NativeRHI', raw: nativeRhi },
    ],
  },
  {
    group: '工具链',
    items: [{ slug: 'cli', title: 'cove CLI 手册', raw: cli }],
  },
]

export const allDocs: DocLeaf[] = docGroups.flatMap((g) => g.items)

export function findDoc(slug?: string): DocLeaf {
  return allDocs.find((d) => d.slug === slug) ?? allDocs[0]
}
