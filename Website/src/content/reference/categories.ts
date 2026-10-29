/**
 * Category blurbs shown on the framework landing page and category headers.
 * Kept in one place so the sidebar, cards and page headers never disagree.
 */
export interface CategoryBlurb {
  id: string
  blurb: string
}

export const categoryBlurbs: Record<string, string> = {
  core: '视图协议、视图构建器、原始容器，以及状态、绑定、几何与颜色等共享值类型。',
  views: '以组合方式构建的示例视图，例如 Pet 场景与模块占位类型。',
  text: 'Text、TextField、富文本与代码编辑相关的文本类型。',
  layout: '布局容器、滚动、分栏、标签页，以及 frame / flex / padding 等布局修饰符。',
  controls: '按钮、输入框、选择器、开关、工具栏与表单控件。',
  navigation: 'NavigationStack、路由出口与步骤器。',
  overlays: '浮层、弹窗、菜单、提示，以及 LayerRoot 门户。',
  data: '列表、表格、树、图表与属性网格。',
  media: '图像、图标、异步图像与骨架占位。',
  feedback: '警告、徽标、空状态、进度与加载指示。',
  animation: '过渡、关键帧、弹簧拖动与 withAnimation。',
  theme: '主题 token、组件样式与环境读取。',
  style: '背景、边框、阴影与表面质感等外观修饰符。',
  interaction: '悬停、光标、命中测试、焦点与无障碍修饰符。',
  graphics: 'NativeRHI 后端、设备与渲染管线类型。',
  shell: '窗口、原生菜单、显示器与平台外壳。',
  runtime: '平台宿主、表面工厂与事件桥。',
  app: '应用配置、启动与内存诊断。',
  devtools: '检查点与开发期诊断。',
  terminal: '终端视图与会话。',
}

/** Sidebar grouping — mirrors the framework's layering, not the file tree. */
export interface NavGroup {
  title: string
  categories: string[]
}

export const navGroups: NavGroup[] = [
  { title: '基础', categories: ['core', 'views'] },
  { title: '界面', categories: ['layout', 'text', 'controls', 'navigation', 'overlays', 'data', 'media', 'feedback'] },
  { title: '外观与动效', categories: ['theme', 'style', 'animation', 'interaction'] },
  { title: '平台', categories: ['app', 'shell', 'runtime', 'graphics', 'terminal', 'devtools'] },
]
