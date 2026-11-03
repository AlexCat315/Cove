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
import cargo from '@/content/cargo.md?raw'
import gallery from '@/content/components/gallery.md?raw'
import slider from '@/content/components/slider.md?raw'
import rangeSlider from '@/content/components/range-slider.md?raw'
import rating from '@/content/components/rating.md?raw'
import searchField from '@/content/components/search-field.md?raw'
import textField from '@/content/components/text-field.md?raw'
import textSelection from '@/content/components/text-selection.md?raw'

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
    group: 'Rust 组件',
    items: [
      { slug: 'gallery', title: 'Gallery 使用', raw: gallery },
      { slug: 'slider', title: 'Slider', raw: slider },
      { slug: 'range-slider', title: 'RangeSlider', raw: rangeSlider },
      { slug: 'rating', title: 'Rating', raw: rating },
      { slug: 'search-field', title: 'SearchField', raw: searchField },
      { slug: 'text-field', title: 'TextField', raw: textField },
      { slug: 'text-selection', title: '文本选择', raw: textSelection },
    ],
  },
  {
    group: '工具链',
    items: [{ slug: 'cargo', title: 'Cargo 工作流', raw: cargo }],
  },
]

export const allDocs: DocLeaf[] = docGroups.flatMap((g) => g.items)

export function findDoc(slug?: string): DocLeaf {
  return allDocs.find((d) => d.slug === slug) ?? allDocs[0]
}
