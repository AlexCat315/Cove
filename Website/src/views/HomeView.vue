<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import WaveBackground from '@/components/WaveBackground.vue'
import CodeBlock from '@/components/CodeBlock.vue'

const heroCode = `import CoveUI

struct CounterView: View {
  @State var count = 0

  var body: some View {
    VStack(spacing: 16) {
      Text("Hello, Cove")
        .font(.largeTitle)
        .foregroundStyle(.cyan)

      Button("点了 \\(count) 次") {
        count += 1
      }
      .padding(.horizontal, 24)
      .cornerRadius(999)
    }
    .padding(32)
  }
}`

const cliLines = [
  { prompt: '$', text: 'cove setup', output: '✓ 工具链检查通过 · Swift 6.4.1' },
  { prompt: '$', text: 'cove create app MyApp', output: '✓ 已生成桌面 / Android / iOS 共享工程' },
  { prompt: '$', text: 'cove run MyApp', output: '▶ 正在构建并启动 NativeRHI 窗口…' },
]

const features = [
  {
    icon: 'M4 6h16M4 12h16M4 18h10',
    title: '声明式视图',
    desc: '用 Swift 描述界面，状态驱动更新。组合即一切，复杂界面也能保持清晰可读。',
  },
  {
    icon: 'M12 3v18M3 12h18',
    title: '弹性布局引擎',
    desc: '基于 Yoga 的 Flexbox 布局，跨平台像素级一致，从手表到桌面从容伸缩。',
  },
  {
    icon: 'M4 4h16v12H4zM2 20h20',
    title: '统一文本管线',
    desc: 'FreeType 光栅化 + HarfBuzz 塑形，复杂脚本、连字与 emoji 排版一次搞定。',
  },
  {
    icon: 'M13 2 3 14h7l-1 8 10-12h-7z',
    title: 'NativeRHI 加速',
    desc: 'Metal、DirectX 12、Vulkan 同一抽象，GPU 合成与离屏渲染全程硬件加速。',
  },
  {
    icon: 'M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3',
    title: '五端一套代码',
    desc: 'macOS、Windows、Linux、Android、iOS 共享视图与业务逻辑，宿主原生呈现。',
  },
  {
    icon: 'M5 12h14M12 5l7 7-7 7',
    title: 'cove 一体化 CLI',
    desc: '创建、运行、测试、构建、SDK 版本管理，一条命令串起整个开发循环。',
  },
]

const platforms = [
  { name: 'macOS', sub: 'Metal · arm64', icon: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z' },
  { name: 'Windows', sub: 'DX12 · x86_64', icon: 'M3 5h8v6H3zM13 5h8v6h-8zM3 13h8v6H3zM13 13h8v6h-8z' },
  { name: 'Linux', sub: 'Vulkan · x86_64', icon: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 7v5l3 3' },
  { name: 'Android', sub: 'Vulkan · arm64', icon: 'M7 7h10v10a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2zM7 7 5 3M17 7l2-4' },
  { name: 'iOS', sub: 'Metal · arm64', icon: 'M8 2h8v20H8zM11 18h2' },
]

const visibleCli = ref(0)
onMounted(() => {
  const timer = setInterval(() => {
    visibleCli.value = (visibleCli.value + 1) % (cliLines.length + 1)
    if (visibleCli.value === 0) {
      clearInterval(timer)
      setTimeout(() => {
        visibleCli.value = 1
      }, 2600)
    }
  }, 900)
})
</script>

<template>
  <WaveBackground />

  <!-- ================= HERO ================= -->
  <section class="hero">
    <div class="container hero-inner">
      <div class="hero-content">
        <div class="hero-badge">
          <span class="pulse-dot"></span>
          Swift 6.4.1 · 五端同源 · Apache 2.0
        </div>
        <h1 class="hero-title">
          一套 Swift 代码<br />
          抵达<span class="gradient-text">每一块屏幕</span>
        </h1>
        <p class="hero-sub">
          Cove 是跨平台声明式 UI 框架与 <code>cove</code> CLI。统一的视图、布局、文本与图像管线，
          通过 NativeRHI 驱动 Metal、DirectX 12 与 Vulkan。
        </p>
        <div class="hero-cta">
          <RouterLink to="/download" class="btn btn-primary">
            下载 SDK
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <path d="M5 12h14M12 5l7 7-7 7" />
            </svg>
          </RouterLink>
          <RouterLink to="/docs/getting-started" class="btn btn-ghost">快速开始</RouterLink>
        </div>
        <div class="hero-stats">
          <div><strong>5</strong><span>目标平台</span></div>
          <div><strong>3</strong><span>GPU 后端</span></div>
          <div><strong>100%</strong><span>Swift 编写</span></div>
        </div>
      </div>

      <div class="hero-code">
        <CodeBlock :code="heroCode" title="CounterView.swift" lang="swift" />
      </div>
    </div>
  </section>

  <!-- ================= FEATURES ================= -->
  <section class="section" id="features">
    <div class="container">
      <div class="reveal">
        <span class="eyebrow">为什么是 Cove</span>
        <h2 class="section-title">为跨平台而生的每一层</h2>
        <p class="section-sub">从视图描述到 GPU 提交，Cove 把跨平台的复杂度收敛在框架内部，把简洁还给开发者。</p>
      </div>
      <div class="feature-grid">
        <div v-for="(f, i) in features" :key="f.title" class="feature-card glass reveal" :data-reveal-delay="i * 80">
          <div class="feature-icon">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path :d="f.icon" />
            </svg>
          </div>
          <h3>{{ f.title }}</h3>
          <p>{{ f.desc }}</p>
        </div>
      </div>
    </div>
  </section>

  <!-- ================= PLATFORMS ================= -->
  <section class="section platforms-section">
    <div class="container">
      <div class="reveal">
        <span class="eyebrow">平台矩阵</span>
        <h2 class="section-title">桌面、移动，原生呈现</h2>
      </div>
      <div class="platform-row">
        <div v-for="(p, i) in platforms" :key="p.name" class="platform-card glass reveal" :data-reveal-delay="i * 90">
          <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            <path :d="p.icon" />
          </svg>
          <strong>{{ p.name }}</strong>
          <span>{{ p.sub }}</span>
        </div>
      </div>
    </div>
  </section>

  <!-- ================= CLI ================= -->
  <section class="section" id="cli">
    <div class="container cli-wrap">
      <div class="cli-copy reveal">
        <span class="eyebrow">cove CLI</span>
        <h2 class="section-title">三条命令，从空目录到运行</h2>
        <p class="section-sub">
          CLI 自带运行库，无需先安装 Swift 即可执行。SDK 版本由 <code>cove.json</code> 固定，
          所有项目共享同一份工具链。
        </p>
        <RouterLink to="/docs/cli" class="btn btn-ghost">查看 CLI 手册</RouterLink>
      </div>
      <div class="cli-terminal glass reveal">
        <div class="term-bar">
          <span class="dot red"></span><span class="dot yellow"></span><span class="dot green"></span>
          <span class="term-title">terminal — cove</span>
        </div>
        <div class="term-body">
          <template v-for="(line, i) in cliLines" :key="i">
            <div v-if="visibleCli > i" class="term-line">
              <span class="term-prompt">{{ line.prompt }}</span>
              <span class="term-cmd">{{ line.text }}</span>
            </div>
            <div v-if="visibleCli > i" class="term-output">{{ line.output }}</div>
          </template>
          <div class="term-cursor"></div>
        </div>
      </div>
    </div>
  </section>

  <!-- ================= FINAL CTA ================= -->
  <section class="section final-cta">
    <div class="container reveal">
      <div class="cta-box glass">
        <h2>准备好驶入 Cove 了吗？</h2>
        <p>安装 SDK，几分钟内跑起你的第一个跨平台应用。</p>
        <div class="hero-cta" style="justify-content: center">
          <RouterLink to="/download" class="btn btn-primary">立即下载</RouterLink>
          <RouterLink to="/showcase" class="btn btn-ghost">看看别人做了什么</RouterLink>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* ---------- HERO ---------- */
.hero {
  position: relative;
  min-height: 100vh;
  display: flex;
  align-items: center;
  padding: calc(var(--nav-h) + 40px) 0 60px;
}

.hero-inner {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: 1.05fr 1fr;
  gap: 56px;
  align-items: center;
}

.hero-badge {
  display: inline-flex;
  align-items: center;
  gap: 9px;
  padding: 7px 16px;
  border-radius: 999px;
  border: 1px solid var(--border-strong);
  background: var(--surface);
  font-size: 13px;
  color: var(--text-dim);
  margin-bottom: 28px;
}

.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--secondary);
  box-shadow: 0 0 0 0 rgba(45, 212, 191, 0.6);
  animation: pulse 2s infinite;
}

@keyframes pulse {
  70% { box-shadow: 0 0 0 9px rgba(45, 212, 191, 0); }
  100% { box-shadow: 0 0 0 0 rgba(45, 212, 191, 0); }
}

.hero-title {
  font-size: clamp(38px, 5.2vw, 62px);
  margin-bottom: 24px;
}

.hero-sub {
  font-size: 17.5px;
  color: var(--text-dim);
  max-width: 540px;
  margin-bottom: 36px;
}
.hero-sub code {
  font-size: 14px;
  color: var(--primary);
  background: rgba(34, 211, 238, 0.1);
  padding: 2px 8px;
  border-radius: 6px;
}

.hero-cta {
  display: flex;
  gap: 14px;
  flex-wrap: wrap;
  margin-bottom: 44px;
}

.hero-stats {
  display: flex;
  gap: 40px;
}
.hero-stats div {
  display: flex;
  flex-direction: column;
}
.hero-stats strong {
  font-size: 26px;
  background: var(--gradient);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}
.hero-stats span {
  font-size: 13px;
  color: var(--text-faint);
}

.hero-code {
  animation: float 7s ease-in-out infinite;
}
@keyframes float {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-14px); }
}

/* ---------- FEATURES ---------- */
.feature-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 22px;
  margin-top: 52px;
}

.feature-card {
  padding: 30px 26px;
  transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1), border-color 0.35s;
}

.feature-card:hover {
  transform: translateY(-6px);
  border-color: var(--border-strong);
}

.feature-icon {
  width: 46px;
  height: 46px;
  border-radius: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, rgba(34, 211, 238, 0.18), rgba(45, 212, 191, 0.12));
  color: var(--primary);
  margin-bottom: 18px;
}

.feature-card h3 {
  font-size: 18px;
  margin-bottom: 9px;
}
.feature-card p {
  color: var(--text-dim);
  font-size: 14.5px;
}

/* ---------- PLATFORMS ---------- */
.platforms-section {
  background: linear-gradient(180deg, transparent, rgba(7, 28, 46, 0.5), transparent);
}

.platform-row {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 16px;
  margin-top: 44px;
}

.platform-card {
  padding: 28px 16px;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  color: var(--text-dim);
  transition: transform 0.3s, border-color 0.3s, color 0.3s;
}
.platform-card:hover {
  transform: translateY(-5px);
  border-color: var(--primary);
  color: var(--primary);
}
.platform-card strong {
  font-size: 15px;
  color: var(--text);
}
.platform-card span {
  font-size: 12px;
  font-family: var(--font-mono);
}

/* ---------- CLI ---------- */
.cli-wrap {
  display: grid;
  grid-template-columns: 1fr 1.1fr;
  gap: 52px;
  align-items: center;
}

.cli-terminal {
  overflow: hidden;
}

.term-bar {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  background: rgba(12, 46, 68, 0.5);
}
.dot {
  width: 11px;
  height: 11px;
  border-radius: 50%;
}
.red { background: #ff5f57; }
.yellow { background: #febc2e; }
.green { background: #28c840; }
.term-title {
  margin-left: 10px;
  font-size: 12px;
  color: var(--text-faint);
}

.term-body {
  padding: 22px;
  font-family: var(--font-mono);
  font-size: 14px;
  min-height: 220px;
}
.term-line {
  display: flex;
  gap: 8px;
}
.term-prompt {
  color: var(--secondary);
}
.term-cmd {
  color: var(--text);
}
.term-output {
  color: var(--text-faint);
  margin: 3px 0 16px;
  font-size: 13px;
}
.term-cursor {
  width: 8px;
  height: 17px;
  background: var(--primary);
  animation: blink 1.1s steps(1) infinite;
}
@keyframes blink {
  50% { opacity: 0; }
}

/* ---------- FINAL CTA ---------- */
.cta-box {
  text-align: center;
  padding: 70px 40px;
}
.cta-box h2 {
  font-size: clamp(26px, 3.5vw, 38px);
  margin-bottom: 14px;
}
.cta-box p {
  color: var(--text-dim);
  margin-bottom: 32px;
}

/* ---------- RESPONSIVE ---------- */
@media (max-width: 980px) {
  .hero-inner {
    grid-template-columns: 1fr;
    gap: 40px;
  }
  .hero-code {
    order: 2;
  }
  .feature-grid {
    grid-template-columns: 1fr 1fr;
  }
  .platform-row {
    grid-template-columns: repeat(3, 1fr);
  }
  .cli-wrap {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 620px) {
  .feature-grid {
    grid-template-columns: 1fr;
  }
  .platform-row {
    grid-template-columns: repeat(2, 1fr);
  }
  .hero-stats {
    gap: 26px;
  }
}
</style>
