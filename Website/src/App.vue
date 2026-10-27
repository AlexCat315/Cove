<script setup lang="ts">
import { onMounted } from 'vue'
import { RouterView } from 'vue-router'
import TheNav from '@/components/TheNav.vue'
import TheFooter from '@/components/TheFooter.vue'
import { revealDirective } from '@/directives/reveal'

const vReveal = revealDirective

onMounted(() => {
  // Ensure proper paint on first load
  document.body.style.opacity = '1'
})
</script>

<template>
  <TheNav />
  <main>
    <RouterView v-slot="{ Component }">
      <transition name="page" mode="out-in">
        <component :is="Component" v-reveal />
      </transition>
    </RouterView>
  </main>
  <TheFooter />
</template>

<style>
body {
  opacity: 0;
  transition: opacity 0.3s ease;
}
</style>
