<script setup lang="ts">
/**
 * WaveBackground — layered animated waves, drifting "plankton" particles
 * and a soft glow that follows the pointer. Pure Canvas 2D, DPR-aware,
 * pauses when the tab is hidden.
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'

const canvas = ref<HTMLCanvasElement | null>(null)
let ctx: CanvasRenderingContext2D | null = null
let raf = 0
let width = 0
let height = 0
let dpr = 1

const pointer = { x: 0, y: 0, tx: 0, ty: 0 }

interface Particle {
  x: number
  y: number
  r: number
  vx: number
  vy: number
  a: number
}
let particles: Particle[] = []

interface WaveLayer {
  amplitude: number
  wavelength: number
  speed: number
  color: string
  fill: string
  base: number
  phase: number
}
let layers: WaveLayer[] = []

function resize() {
  if (!canvas.value) return
  dpr = Math.min(window.devicePixelRatio || 1, 2)
  width = window.innerWidth
  height = window.innerHeight
  canvas.value.width = width * dpr
  canvas.value.height = height * dpr
  canvas.value.style.width = `${width}px`
  canvas.value.style.height = `${height}px`
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0)
  buildScene()
}

function buildScene() {
  layers = [
    {
      amplitude: 26,
      wavelength: 340,
      speed: 0.55,
      color: 'rgba(34, 211, 238, 0.0)',
      fill: 'rgba(34, 211, 238, 0.05)',
      base: 0.62,
      phase: 0,
    },
    {
      amplitude: 34,
      wavelength: 460,
      speed: 0.38,
      color: '',
      fill: 'rgba(45, 212, 191, 0.07)',
      base: 0.72,
      phase: 2.1,
    },
    {
      amplitude: 42,
      wavelength: 600,
      speed: 0.24,
      color: '',
      fill: 'rgba(56, 189, 248, 0.09)',
      base: 0.84,
      phase: 4.4,
    },
  ]

  const count = Math.min(90, Math.floor((width * height) / 22000))
  particles = Array.from({ length: count }, () => ({
    x: Math.random() * width,
    y: Math.random() * height,
    r: Math.random() * 1.8 + 0.4,
    vx: (Math.random() - 0.5) * 0.25,
    vy: -(Math.random() * 0.35 + 0.08),
    a: Math.random() * 0.5 + 0.15,
  }))
}

function drawWave(layer: WaveLayer, t: number) {
  if (!ctx) return
  ctx.beginPath()
  ctx.moveTo(0, height)
  const baseY = height * layer.base
  for (let x = 0; x <= width; x += 8) {
    const y =
      baseY +
      Math.sin((x / layer.wavelength) * Math.PI * 2 + t * layer.speed + layer.phase) *
        layer.amplitude +
      Math.sin((x / (layer.wavelength * 0.6)) * Math.PI * 2 - t * layer.speed * 1.4) *
        layer.amplitude *
        0.35
    ctx.lineTo(x, y)
  }
  ctx.lineTo(width, height)
  ctx.closePath()
  ctx.fillStyle = layer.fill
  ctx.fill()
}

function frame(t: number) {
  if (!ctx) return
  ctx.clearRect(0, 0, width, height)

  // pointer glow (eased)
  pointer.x += (pointer.tx - pointer.x) * 0.06
  pointer.y += (pointer.ty - pointer.y) * 0.06
  const glow = ctx.createRadialGradient(pointer.x, pointer.y, 0, pointer.x, pointer.y, 320)
  glow.addColorStop(0, 'rgba(34, 211, 238, 0.10)')
  glow.addColorStop(1, 'rgba(34, 211, 238, 0)')
  ctx.fillStyle = glow
  ctx.fillRect(0, 0, width, height)

  // particles
  for (const p of particles) {
    p.x += p.vx
    p.y += p.vy
    if (p.y < -10) {
      p.y = height + 10
      p.x = Math.random() * width
    }
    if (p.x < -10) p.x = width + 10
    if (p.x > width + 10) p.x = -10
    ctx.beginPath()
    ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2)
    ctx.fillStyle = `rgba(103, 232, 249, ${p.a})`
    ctx.fill()
  }

  layers.forEach((l) => {
    l.phase += 0
    drawWave(l, t / 1000)
  })

  raf = requestAnimationFrame(frame)
}

function onPointerMove(e: PointerEvent) {
  pointer.tx = e.clientX
  pointer.ty = e.clientY
}

function onVisibility() {
  if (document.hidden) {
    cancelAnimationFrame(raf)
  } else {
    raf = requestAnimationFrame(frame)
  }
}

onMounted(() => {
  if (!canvas.value) return
  ctx = canvas.value.getContext('2d')
  resize()
  window.addEventListener('resize', resize)
  window.addEventListener('pointermove', onPointerMove)
  document.addEventListener('visibilitychange', onVisibility)
  raf = requestAnimationFrame(frame)
})

onBeforeUnmount(() => {
  cancelAnimationFrame(raf)
  window.removeEventListener('resize', resize)
  window.removeEventListener('pointermove', onPointerMove)
  document.removeEventListener('visibilitychange', onVisibility)
})
</script>

<template>
  <canvas ref="canvas" class="wave-canvas" aria-hidden="true"></canvas>
</template>

<style scoped>
.wave-canvas {
  position: fixed;
  inset: 0;
  z-index: 0;
  pointer-events: none;
}
</style>
