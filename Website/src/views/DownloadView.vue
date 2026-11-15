<script setup lang="ts">
import { ref } from 'vue'

const copied = ref(false)
const setup = `git clone --branch v0.3.0 --recurse-submodules https://github.com/AlexCat315/Cove.git
cd Cove
cargo cove setup
cargo run -p cove-examples --bin gallery --locked`

async function copyCmd() {
  await navigator.clipboard.writeText(setup)
  copied.value = true
  setTimeout(() => (copied.value = false), 1600)
}
</script>

<template>
  <div class="download-page">
    <section class="dl-hero">
      <div class="container">
        <span class="eyebrow reveal">下载</span>
        <h1 class="reveal" style="font-size: clamp(32px, 4.5vw, 50px)">获取 Cove SDK 0.3.0</h1>
        <p class="reveal" style="color: var(--text-dim); max-width: 620px; font-size: 17px">
          安装 Git 和 rustup 后，在 macOS、Windows 或 Linux 从源码构建并运行 Gallery。
        </p>
      </div>
    </section>

    <section class="container dl-body">
      <div class="install-card glass reveal">
        <div class="install-head">
          <span class="install-file">终端</span>
          <button class="copy-cmd" @click="copyCmd">
            {{ copied ? '已复制 ✓' : '复制命令' }}
          </button>
        </div>
        <pre class="install-cmd">{{ setup }}</pre>
      </div>

      <div class="notes glass reveal">
        <h3>开始开发</h3>
        <ul>
          <li>Rust 版本由 <code>rust-toolchain.toml</code> 锁定。<code>cargo cove setup</code> 准备本机原生构建工具。</li>
          <li>通过仓库的 <code>Templates/app</code> 生成应用。平台说明、模板和迁移步骤见发布说明。</li>
        </ul>
        <a class="gh-link" href="https://github.com/AlexCat315/Cove/releases/tag/v0.3.0" target="_blank" rel="noopener">
          查看 Rust SDK 0.3.0
        </a>
        <br>
        <a class="gh-link" href="https://github.com/AlexCat315/Cove/releases/tag/v0.2.13" target="_blank" rel="noopener">
          下载 Swift SDK 0.2.13
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

</style>
