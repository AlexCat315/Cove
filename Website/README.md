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
| API 参考 | 源码提取的 JSON（`scripts/extract-api.py`）|

## 快速开始

```bash
npm install
npm run dev        # 启动开发服务器 http://localhost:5173
```

## 构建与预览

```bash
npm run build      # 类型检查 + 生产构建（输出到 dist/）
npm run preview    # 本地预览生产构建
npm run docs:api   # 从 ../ 的 Swift 源码重新生成 API 参考数据
```

## 目录结构

```
Website/
├── public/
│   └── favicon.svg            # 站点图标
├── src/
│   ├── components/
│   │   ├── reference/
│   │   │   └── RefCode.vue    # 声明代码块（高亮 + 复制 + 自动换行）
│   │   ├── TheNav.vue         # 顶部导航（滚动玻璃态、移动端抽屉）
│   │   ├── TheFooter.vue      # 页脚
│   │   ├── WaveBackground.vue # 交互式水波 Canvas 背景
│   │   └── CodeBlock.vue      # 代码块（语法高亮 + 复制）
│   ├── content/               # 文档与参考数据
│   │   ├── getting-started.md
│   │   ├── architecture.md
│   │   ├── views.md
│   │   ├── native-rhi.md
│   │   ├── cli.md
│   │   ├── api/               # 生成产物：index.json + 每个分类一个 JSON
│   │   └── reference/         # 手写概述（framework.md、symbols/*.md、categories.ts）
│   ├── directives/
│   │   └── reveal.ts          # v-reveal 滚动入场指令
│   ├── router/
│   │   └── index.ts           # 路由表与页面标题
│   ├── styles/
│   │   └── main.css           # 设计系统（Deep Bay 主题变量）
│   ├── utils/
│   │   ├── reference.ts       # 参考站数据访问、分类分组、搜索
│   │   ├── docs.ts            # 文档目录清单（侧边栏树）
│   │   ├── highlight.ts       # 轻量语法高亮
│   │   └── markdown.ts        # marked 渲染（代码块 / 标题锚点 / TOC）
│   ├── views/
│   │   ├── HomeView.vue       # 首页
│   │   ├── DocsView.vue       # 文档站（三栏布局）
│   │   ├── ReferenceView.vue  # API 参考（框架 / 分类 / 符号三种页面）
│   │   ├── DownloadView.vue   # 下载页
│   │   ├── ShowcaseView.vue   # 应用展示
│   │   └── NotFoundView.vue   # 404
│   ├── App.vue
│   ├── env.d.ts
│   └── main.ts
├── scripts/
│   └── extract-api.py         # 从 Swift 源码提取公开符号
├── index.html
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## 页面与功能

- **首页**：水波粒子背景、Hero 代码窗口、六大特性、五平台矩阵、CLI 终端逐行演示、CTA。
- **文档站**：三栏布局（侧边栏目录 / 正文 / 页内 TOC），Markdown 渲染、代码块复制、上一页/下一页、移动端抽屉。
- **API 参考**（`/documentation/coveui`）：仿 Apple 文档结构的三栏参考站，框架概览、分类索引与符号页，带声明、概述、主题、关系、另请参阅、源码定位与符号搜索。
- **下载页**：平台切换、安装命令复制、四步安装引导、校验说明。
- **展示页**：六类应用场景的界面模拟（带动效）。
- **404**：海湾主题动效页。

## 设计与动效

- **主题：Deep Bay（深海湾）**——深海蓝黑底、青色/松石绿光效、玻璃拟态卡片。
- 多层波浪 + 漂浮粒子 + 鼠标光晕的 Canvas 背景，DPR 自适应，页面隐藏时自动暂停。
- 滚动入场（`v-reveal`）、Hero 代码窗口悬浮、终端逐行打字、数字与脉冲动画。
- 全站响应式，覆盖桌面与移动端。

## 如何修改文档

指南：直接编辑 `src/content/*.md`，并在 `src/utils/docs.ts` 中登记侧边栏条目。新增代码块会自动获得语法高亮与复制按钮。

## API 参考如何生成

参考站的符号页不是手写的。`scripts/extract-api.py` 扫描仓库里的 Swift 源码，把 `public` 类型、成员与 `View` 扩展方法连同声明、文档注释、文件名与行号导出到 `src/content/api/*.json`：

```bash
npm run docs:api
```

- 分类（`core`、`layout`、`text`…）由源码目录决定，标题在脚本的 `CATEGORY_TITLES` 中维护。
- 同名 `View` 扩展方法会合并成一个页面，多个重载并列显示。
- 手写内容只负责“概述”和分类导语，位于 `src/content/reference/`；没有手写概述的符号会回退到源码文档注释，再回退到空状态说明。
- 修改 Swift API 后重新运行脚本即可，不需要改动页面代码。

## 许可

与上游 Cove 一致，采用 Apache License 2.0。
