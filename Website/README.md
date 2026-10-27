# Cove Website

Cove 官方网站，使用 **Vue 3 + TypeScript + Vite** 构建。

> 上游项目：[Cove](../) — Swift 跨平台 UI 框架与 `cove` CLI，通过 NativeRHI 驱动 Metal、DirectX 12 与 Vulkan。

## 技术栈

| 类别 | 选型 |
|------|------|
| 框架 | Vue 3（`<script setup>`） |
| 语言 | TypeScript 5（strict） |
| 构建 | Vite 5 |
| 路由 | Vue Router 4 |
| 状态 | Pinia |
| 动效 | GSAP、CSS 动画、Canvas 2D |
| 文档 | Markdown（`?raw` 导入）+ marked 渲染 |

## 快速开始

```bash
npm install
npm run dev        # 启动开发服务器 http://localhost:5173
```

## 构建与预览

```bash
npm run build      # 类型检查 + 生产构建（输出到 dist/）
npm run preview    # 本地预览生产构建
```

## 目录结构

```
Website/
├── public/
│   └── favicon.svg            # 站点图标
├── src/
│   ├── components/
│   │   ├── TheNav.vue         # 顶部导航（滚动玻璃态、移动端抽屉）
│   │   ├── TheFooter.vue      # 页脚
│   │   ├── WaveBackground.vue # 交互式水波 Canvas 背景
│   │   └── CodeBlock.vue      # 代码块（语法高亮 + 复制）
│   ├── content/               # 文档 Markdown 源文件
│   │   ├── getting-started.md
│   │   ├── architecture.md
│   │   ├── views.md
│   │   ├── native-rhi.md
│   │   └── cli.md
│   ├── directives/
│   │   └── reveal.ts          # v-reveal 滚动入场指令
│   ├── router/
│   │   └── index.ts           # 路由表与页面标题
│   ├── styles/
│   │   └── main.css           # 设计系统（Deep Bay 主题变量）
│   ├── utils/
│   │   ├── docs.ts            # 文档目录清单（侧边栏树）
│   │   ├── highlight.ts       # 轻量语法高亮
│   │   └── markdown.ts        # marked 渲染（代码块 / 标题锚点 / TOC）
│   ├── views/
│   │   ├── HomeView.vue       # 首页
│   │   ├── DocsView.vue       # 文档站（三栏布局）
│   │   ├── DownloadView.vue   # 下载页
│   │   ├── ShowcaseView.vue   # 应用展示
│   │   └── NotFoundView.vue   # 404
│   ├── App.vue
│   ├── env.d.ts
│   └── main.ts
├── index.html
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## 页面与功能

- **首页**：水波粒子背景、Hero 代码窗口、六大特性、五平台矩阵、CLI 终端逐行演示、CTA。
- **文档站**：三栏布局（侧边栏目录 / 正文 / 页内 TOC），Markdown 渲染、代码块复制、上一页/下一页、移动端抽屉。
- **下载页**：平台切换、安装命令复制、四步安装引导、校验说明。
- **展示页**：六类应用场景的界面模拟（带动效）。
- **404**：海湾主题动效页。

## 设计与动效

- **主题：Deep Bay（深海湾）**——深海蓝黑底、青色/松石绿光效、玻璃拟态卡片。
- 多层波浪 + 漂浮粒子 + 鼠标光晕的 Canvas 背景，DPR 自适应，页面隐藏时自动暂停。
- 滚动入场（`v-reveal`）、Hero 代码窗口悬浮、终端逐行打字、数字与脉冲动画。
- 全站响应式，覆盖桌面与移动端。

## 如何修改文档

直接编辑 `src/content/*.md`，并在 `src/utils/docs.ts` 中登记侧边栏条目即可。新增代码块会自动获得语法高亮与复制按钮。

## 许可

与上游 Cove 一致，采用 Apache License 2.0。
