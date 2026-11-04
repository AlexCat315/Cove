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
import textFieldStyle from '@/content/components/text-field-style.md?raw'
import textBuffer from '@/content/components/text-buffer.md?raw'
import textSelection from '@/content/components/text-selection.md?raw'
import theme from '@/content/components/theme.md?raw'
import splitButton from '@/content/components/split-button.md?raw'
import completionPopover from '@/content/components/completion-popover.md?raw'
import numberField from '@/content/components/number-field.md?raw'
import field from '@/content/components/field.md?raw'
import colorWheel from '@/content/components/color-wheel.md?raw'
import select from '@/content/components/select.md?raw'
import colorField from '@/content/components/color-field.md?raw'
import textArea from '@/content/components/text-area.md?raw'
import textEditor from '@/content/components/text-editor.md?raw'
import text from '@/content/components/text.md?raw'

import component0 from '@/content/components/sidebar.md?raw'
import component1 from '@/content/components/navigation-drawer.md?raw'
import component2 from '@/content/components/workspace.md?raw'
import component3 from '@/content/components/scroll-view.md?raw'
import component4 from '@/content/components/scrollbar.md?raw'
import component5 from '@/content/components/route-outlet.md?raw'
import component6 from '@/content/components/async-image.md?raw'
import component7 from '@/content/components/hover-card.md?raw'
import component8 from '@/content/components/command-palette.md?raw'
import component9 from '@/content/components/menu.md?raw'
import component10 from '@/content/components/toolbar.md?raw'
import component11 from '@/content/components/vec3-field.md?raw'
import component12 from '@/content/components/timeline.md?raw'
import chart from '@/content/components/chart.md?raw'
import table from '@/content/components/table.md?raw'
import tree from '@/content/components/tree.md?raw'
import dragAndDrop from '@/content/components/drag-and-drop.md?raw'
import component13 from '@/content/components/tab-view.md?raw'
import component14 from '@/content/components/date-range.md?raw'

import actionGroupLayout from '@/content/components/action-group-layout.md?raw'
import markdown from '@/content/components/markdown.md?raw'
import grid from '@/content/components/grid.md?raw'
import terminal from '@/content/components/terminal.md?raw'
import resourcePreparation from '@/content/components/resource-preparation.md?raw'
import dragUpload from '@/content/components/drag-upload.md?raw'

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
      { slug: 'resource-preparation', title: 'Resource preparation', raw: resourcePreparation },
      { slug: 'theme', title: '默认主题', raw: theme },
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
      { slug: 'text-field-style', title: 'TextFieldStyle', raw: textFieldStyle },
      { slug: 'text-buffer', title: 'TextBuffer', raw: textBuffer },
      { slug: 'text-selection', title: '文本选择', raw: textSelection },
      { slug: 'text', title: 'Text', raw: text },
      { slug: 'terminal', title: 'Terminal', raw: terminal },
      { slug: 'sidebar', title: 'Sidebar / AdaptiveSidebar', raw: component0 },
      { slug: 'navigation-drawer', title: 'NavigationDrawer', raw: component1 },
      { slug: 'workspace', title: 'WorkspaceView', raw: component2 },
      { slug: 'scroll-view', title: 'ScrollView', raw: component3 },
      { slug: 'scrollbar', title: 'Scrollbar', raw: component4 },
      { slug: 'route-outlet', title: 'RouteOutlet', raw: component5 },
      { slug: 'async-image', title: 'AsyncImage', raw: component6 },
      { slug: 'hover-card', title: 'HoverCard', raw: component7 },
      { slug: 'command-palette', title: 'CommandPalette', raw: component8 },
      { slug: 'menu', title: 'Menu / MenuItemContent', raw: component9 },
      { slug: 'split-button', title: 'SplitButton', raw: splitButton },
      { slug: 'completion-popover', title: 'CompletionPopover', raw: completionPopover },
      { slug: 'number-field', title: 'NumberField', raw: numberField },
      { slug: 'field', title: 'Field', raw: field },
      { slug: 'select', title: 'Select', raw: select },
      { slug: 'color-field', title: 'ColorField', raw: colorField },
      { slug: 'color-wheel', title: 'ColorWheel', raw: colorWheel },
      { slug: 'text-area', title: 'TextArea / ResizableTextArea', raw: textArea },
      { slug: 'text-editor', title: 'Text Editor', raw: textEditor },
      { slug: 'toolbar', title: 'Toolbar / AppBar', raw: component10 },
      { slug: 'action-group-layout', title: 'ActionGroupLayout', raw: actionGroupLayout },
      { slug: 'drag-upload', title: 'DragUpload / FileDropZone', raw: dragUpload },
      { slug: 'grid', title: 'Grid / named columns', raw: grid },
      { slug: 'markdown', title: 'MarkdownView', raw: markdown },
      { slug: 'vec3-field', title: 'Vec3Field', raw: component11 },
      { slug: 'chart', title: 'Chart', raw: chart },
      { slug: 'table', title: 'Table', raw: table },
      { slug: 'tree', title: 'Tree', raw: tree },
      { slug: 'drag-and-drop', title: 'Drag & Drop', raw: dragAndDrop },
      { slug: 'timeline', title: 'Timeline', raw: component12 },
      { slug: 'tab-view', title: 'TabView', raw: component13 },
      { slug: 'date-range', title: 'DateRangeCalendar', raw: component14 },
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
