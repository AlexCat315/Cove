<script setup lang="ts">
import { ref } from 'vue'

type Platform = 'mac' | 'windows' | 'linux'

const active = ref<Platform>('mac')
const copied = ref(false)

const installers: Record<Platform, { label: string; sub: string; cmd: string; ext: string }> = {
  mac: {
    label: 'macOS',
    sub: 'arm64 · Metal · macOS 26+',
    ext: 'install.sh',
    cmd: `curl -fsSL https://github.com/AlexCat315/Cove/releases/latest/download/install.sh -o install-cove.sh
sh install-cove.sh`,
  },
  windows: {
    label: 'Windows',
    sub: 'x86_64 · DirectX 12 · Windows 10+',
    ext: 'install.ps1',
    cmd: `Invoke-WebRequest https://github.com/AlexCat315/Cove/releases/latest/download/install.ps1 -OutFile install-cove.ps1
./install-cove.ps1`,
  },
  linux: {
    label: 'Linux',
    sub: 'x86_64 · Vulkan',
    ext: 'install.sh',
    cmd: `curl -fsSL https://github.com/AlexCat315/Cove/releases/latest/download/install.sh -o install-cove.sh
sh install-cove.sh`,
  },
}

async function copyCmd() {
  await navigator.clipboard.writeText(installers[active.value].cmd)
  copied.value = true
  setTimeout(() => (copied.value = false), 1600)
}

const steps = [
  { n: '01', t: '运行安装器', d: '安装器下载 CLI、框架源码与主机原生库，并自动配置 PATH。' },
  { n: '02', t: '打开新终端', d: '让 PATH 配置生效，执行 cove --version 验证。' },
  { n: '03', t: 'cove setup', d: '检查 Swift 6.4.1 与平台工具链，按提示补齐缺失项。' },
  { n: '04', t: '创建应用', d: 'cove create app MyApp && cove run MyApp，开始你的旅程。' },
]

const notes = [
  'CLI 自带运行库，仅执行 cove --version 无需先安装 Swift。',
  'cove self update 会校验发布包 SHA-256，再切换到独立版本目录。',
  'Android 与 iOS 产物在首次构建对应目标时按需下载到同一共享 SDK。',
]
</script>

<template>
  <div class="download-page">
    <section class="dl-hero">
      <div class="container">
        <span class="eyebrow reveal">下载</span>
        <h1 class="reveal" style="font-size: clamp(32px, 4.5vw, 50px)">获取 Cove SDK</h1>
        <p class="reveal" style="color: var(--text-dim); max-width: 620px; font-size: 17px">
          选择你的平台，一条命令完成安装。SDK 包含 cove CLI、框架源码与主机原生库。
        </p>
      </div>
    </section>

    <section class="container dl-body">
      <div class="platform-switch reveal">
        <button
          v-for="p in (['mac', 'windows', 'linux'] as Platform[])"
          :key="p"
          class="switch-btn"
          :class="{ active: active === p }"
          @click="active = p"
        >
          <strong>{{ installers[p].label }}</strong>
          <span>{{ installers[p].sub }}</span>
        </button>
      </div>

      <div class="install-card glass reveal">
        <div class="install-head">
          <span class="install-file">{{ installers[active].ext }}</span>
          <button class="copy-cmd" @click="copyCmd">
            {{ copied ? '已复制 ✓' : '复制命令' }}
          </button>
        </div>
        <pre class="install-cmd">{{ installers[active].cmd }}</pre>
      </div>

      <div class="steps-grid">
        <div v-for="(s, i) in steps" :key="s.n" class="step-card glass reveal" :data-reveal-delay="i * 80">
          <span class="step-num">{{ s.n }}</span>
          <h3>{{ s.t }}</h3>
          <p>{{ s.d }}</p>
        </div>
      </div>

      <div class="notes glass reveal">
        <h3>安装说明</h3>
        <ul>
          <li v-for="n in notes" :key="n">{{ n }}</li>
        </ul>
        <a class="gh-link" href="https://github.com/AlexCat315/Cove/releases" target="_blank" rel="noopener">
          查看全部 Release 与校验和 →
        </a>
      </div>
    </section>
  </div>
</template>

<style scoped>
.download-page {
  padding-top: var(--nav-h);
}
.dl-hero {
  padding: 60px 0 30px;
}
.dl-body {
  padding-bottom: 100px;
}

.platform-switch {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
  margin: 30px 0 22px;
}

.switch-btn {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 20px;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--surface);
  cursor: pointer;
  text-align: left;
  color: var(--text);
  transition: border-color 0.25s, background 0.25s, transform 0.25s;
}
.switch-btn span {
  font-size: 12px;
  color: var(--text-faint);
  font-family: var(--font-mono);
}
.switch-btn:hover {
  transform: translateY(-3px);
  border-color: var(--border-strong);
}
.switch-btn.active {
  border-color: var(--primary);
  background: rgba(34, 211, 238, 0.1);
}

.install-card {
  overflow: hidden;
  margin-bottom: 40px;
}
.install-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 20px;
  border-bottom: 1px solid var(--border);
  background: rgba(12, 46, 68, 0.5);
}
.install-file {
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--text-faint);
}
.copy-cmd {
  background: none;
  border: 1px solid var(--border-strong);
  color: var(--text-dim);
  padding: 6px 14px;
  border-radius: 8px;
  font-size: 13px;
  cursor: pointer;
  transition: color 0.2s, border-color 0.2s;
}
.copy-cmd:hover {
  color: var(--primary);
  border-color: var(--primary);
}
.install-cmd {
  padding: 24px 28px;
  font-family: var(--font-mono);
  font-size: 14px;
  line-height: 1.8;
  color: #cbe8f3;
  white-space: pre-wrap;
}

.steps-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
  margin-bottom: 40px;
}
.step-card {
  padding: 26px 22px;
}
.step-num {
  font-family: var(--font-mono);
  font-size: 22px;
  font-weight: 700;
  color: var(--primary);
  opacity: 0.6;
}
.step-card h3 {
  font-size: 16px;
  margin: 12px 0 8px;
}
.step-card p {
  font-size: 13.5px;
  color: var(--text-dim);
}

.notes {
  padding: 32px 36px;
}
.notes h3 {
  margin-bottom: 14px;
}
.notes ul {
  padding-left: 22px;
  color: var(--text-dim);
  font-size: 14.5px;
}
.notes li {
  margin: 8px 0;
}
.gh-link {
  display: inline-block;
  margin-top: 16px;
  color: var(--primary);
  font-size: 14px;
}

@media (max-width: 900px) {
  .steps-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}
@media (max-width: 600px) {
  .platform-switch {
    grid-template-columns: 1fr;
  }
  .steps-grid {
    grid-template-columns: 1fr;
  }
}
</style>
