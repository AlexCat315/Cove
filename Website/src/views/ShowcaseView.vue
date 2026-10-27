<script setup lang="ts">
import { RouterLink } from 'vue-router'

const items = [
  {
    name: 'Counter',
    tag: '入门示例',
    desc: '最小可用的状态驱动应用，展示 @State 与按钮交互。',
    preview: 'counter',
  },
  {
    name: '组件画廊',
    tag: 'CoveUI Gallery',
    desc: '按钮、滑块、列表、开关等全部组件的交互式目录。',
    preview: 'gallery',
  },
  {
    name: 'Mobile 应用',
    tag: '移动端',
    desc: '手机形态的布局与触摸交互，桌面与移动端共享代码。',
    preview: 'mobile',
  },
  {
    name: '文本编辑器',
    tag: '文本管线',
    desc: '多段文本、自定义字体与语法着色的编辑场景。',
    preview: 'editor',
  },
  {
    name: '终端',
    tag: '系统能力',
    desc: '基于 CoveSystem 与文本管线构建的终端界面。',
    preview: 'terminal',
  },
  {
    name: 'Diffuse 工作站',
    tag: '生态项目',
    desc: 'Cove 作为 UI 底座，支撑完整的创作工作站。',
    preview: 'studio',
  },
]
</script>

<template>
  <div class="showcase-page">
    <section class="sc-hero">
      <div class="container">
        <span class="eyebrow reveal">展示</span>
        <h1 class="reveal" style="font-size: clamp(32px, 4.5vw, 50px)">用 Cove 构建的一切</h1>
        <p class="reveal" style="color: var(--text-dim); max-width: 640px; font-size: 17px">
          从最小示例到完整工作站，看看同一套框架如何支撑不同形态的应用。
        </p>
      </div>
    </section>

    <section class="container sc-grid-section">
      <div class="sc-grid">
        <div v-for="(it, i) in items" :key="it.name" class="sc-card glass reveal" :data-reveal-delay="(i % 3) * 90">
          <div class="sc-preview">
            <!-- Counter -->
            <div v-if="it.preview === 'counter'" class="mock-counter">
              <div class="mock-num">42</div>
              <div class="mock-btn">加一</div>
            </div>

            <!-- Gallery -->
            <div v-else-if="it.preview === 'gallery'" class="mock-gallery">
              <span v-for="n in 8" :key="n" class="mock-chip" :style="{ animationDelay: `${n * 0.15}s` }"></span>
            </div>

            <!-- Mobile -->
            <div v-else-if="it.preview === 'mobile'" class="mock-mobile">
              <div class="mock-notch"></div>
              <div class="mock-line w90"></div>
              <div class="mock-line w70"></div>
              <div class="mock-line w80"></div>
              <div class="mock-block"></div>
            </div>

            <!-- Editor -->
            <div v-else-if="it.preview === 'editor'" class="mock-editor">
              <div class="mock-codeline" v-for="n in 5" :key="n" :style="{ width: `${60 + ((n * 13) % 35)}%` }"></div>
            </div>

            <!-- Terminal -->
            <div v-else-if="it.preview === 'terminal'" class="mock-terminal">
              <div class="mock-termline" v-for="n in 4" :key="n">
                <span class="mock-prompt">$</span><span class="mock-termbar" :style="{ width: `${40 + ((n * 17) % 45)}%` }"></span>
              </div>
            </div>

            <!-- Studio -->
            <div v-else class="mock-studio">
              <div class="mock-viewport"></div>
              <div class="mock-panel">
                <div class="mock-line w80" v-for="n in 3" :key="n"></div>
              </div>
            </div>
          </div>

          <div class="sc-meta">
            <div>
              <h3>{{ it.name }}</h3>
              <span class="sc-tag">{{ it.tag }}</span>
            </div>
            <p>{{ it.desc }}</p>
          </div>
        </div>
      </div>

      <div class="sc-cta glass reveal">
        <h2>下一个就是你的作品</h2>
        <p>下载 SDK，十几行代码即可构建第一个应用。</p>
        <RouterLink to="/download" class="btn btn-primary">开始使用</RouterLink>
      </div>
    </section>
  </div>
</template>

<style scoped>
.showcase-page {
  padding-top: var(--nav-h);
}
.sc-hero {
  padding: 60px 0 20px;
}
.sc-grid-section {
  padding-bottom: 100px;
}
.sc-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 22px;
  margin: 40px 0 60px;
}

.sc-card {
  overflow: hidden;
  transition: transform 0.35s, border-color 0.35s;
}
.sc-card:hover {
  transform: translateY(-6px);
  border-color: var(--border-strong);
}

.sc-preview {
  height: 190px;
  padding: 22px;
  background:
    radial-gradient(120% 120% at 0% 0%, rgba(34, 211, 238, 0.08), transparent 60%),
    #030d18;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.sc-meta {
  padding: 22px 24px;
}
.sc-meta h3 {
  font-size: 17px;
  display: inline-block;
  margin-right: 10px;
}
.sc-tag {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--primary);
  background: rgba(34, 211, 238, 0.1);
  padding: 2px 9px;
  border-radius: 999px;
}
.sc-meta p {
  margin-top: 10px;
  font-size: 13.5px;
  color: var(--text-dim);
}

/* ---- mock widgets ---- */
.mock-num {
  font-size: 44px;
  font-weight: 800;
  font-family: var(--font-mono);
  color: var(--primary);
  text-align: center;
}
.mock-btn {
  margin-top: 10px;
  background: var(--gradient);
  color: #03262c;
  font-weight: 700;
  font-size: 13px;
  padding: 7px 22px;
  border-radius: 999px;
  text-align: center;
}
.mock-counter {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.mock-gallery {
  display: flex;
  flex-wrap: wrap;
  gap: 9px;
  justify-content: center;
  width: 100%;
}
.mock-chip {
  width: 42px;
  height: 26px;
  border-radius: 8px;
  background: linear-gradient(135deg, rgba(34, 211, 238, 0.35), rgba(45, 212, 191, 0.2));
  border: 1px solid var(--border-strong);
  animation: chipPulse 2.4s ease-in-out infinite;
}
@keyframes chipPulse {
  50% { opacity: 0.45; }
}

.mock-mobile {
  width: 92px;
  height: 150px;
  border: 2px solid var(--border-strong);
  border-radius: 16px;
  padding: 16px 10px 10px;
  display: flex;
  flex-direction: column;
  gap: 7px;
}
.mock-notch {
  width: 34px;
  height: 4px;
  border-radius: 4px;
  background: var(--border-strong);
  margin: 0 auto 8px;
}
.mock-line {
  height: 6px;
  border-radius: 4px;
  background: rgba(103, 232, 249, 0.3);
}
.w90 { width: 90%; } .w80 { width: 80%; } .w70 { width: 70%; }
.mock-block {
  flex: 1;
  border-radius: 8px;
  background: linear-gradient(135deg, rgba(34, 211, 238, 0.25), rgba(45, 212, 191, 0.12));
}

.mock-editor {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 9px;
}
.mock-codeline {
  height: 8px;
  border-radius: 4px;
  background: rgba(103, 232, 249, 0.28);
}
.mock-codeline:nth-child(3n) {
  background: rgba(45, 212, 191, 0.25);
}

.mock-terminal {
  width: 100%;
  font-family: var(--font-mono);
}
.mock-termline {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}
.mock-prompt {
  color: var(--secondary);
  font-size: 13px;
}
.mock-termbar {
  height: 7px;
  border-radius: 4px;
  background: rgba(103, 232, 249, 0.3);
}

.mock-studio {
  display: flex;
  gap: 12px;
  width: 100%;
  height: 100%;
}
.mock-viewport {
  flex: 1.4;
  border-radius: 10px;
  background:
    linear-gradient(135deg, rgba(34, 211, 238, 0.18), transparent 55%),
    repeating-linear-gradient(0deg, rgba(103, 232, 249, 0.08) 0 1px, transparent 1px 18px),
    repeating-linear-gradient(90deg, rgba(103, 232, 249, 0.08) 0 1px, transparent 1px 18px),
    #051525;
  border: 1px solid var(--border);
}
.mock-panel {
  flex: 1;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: rgba(12, 46, 68, 0.4);
  padding: 14px 10px;
  display: flex;
  flex-direction: column;
  gap: 9px;
  justify-content: center;
}

.sc-cta {
  text-align: center;
  padding: 60px 30px;
}
.sc-cta h2 {
  font-size: 28px;
  margin-bottom: 10px;
}
.sc-cta p {
  color: var(--text-dim);
  margin-bottom: 26px;
}

@media (max-width: 980px) {
  .sc-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}
@media (max-width: 620px) {
  .sc-grid {
    grid-template-columns: 1fr;
  }
}
</style>
