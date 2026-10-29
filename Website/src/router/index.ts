import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: () => import('@/views/HomeView.vue'),
      meta: { title: 'Cove · Swift 跨平台 UI 框架' },
    },
    {
      path: '/showcase',
      name: 'showcase',
      component: () => import('@/views/ShowcaseView.vue'),
      meta: { title: '应用展示 · Cove' },
    },
    {
      path: '/download',
      name: 'download',
      component: () => import('@/views/DownloadView.vue'),
      meta: { title: '下载 SDK · Cove' },
    },
    {
      path: '/docs/:slug?',
      name: 'docs',
      component: () => import('@/views/DocsView.vue'),
      meta: { title: '文档 · Cove' },
    },
    {
      path: '/documentation',
      redirect: '/documentation/coveui',
    },
    {
      path: '/documentation/coveui/:category?/:symbol?',
      name: 'reference',
      component: () => import('@/views/ReferenceView.vue'),
      meta: { title: 'CoveUI 参考 · Cove' },
    },
    {
      path: '/:pathMatch(.*)*',
      name: 'not-found',
      component: () => import('@/views/NotFoundView.vue'),
      meta: { title: '页面未找到 · Cove' },
    },
  ],
  scrollBehavior(to, _from, savedPosition) {
    if (savedPosition) return savedPosition
    if (to.hash) return { el: to.hash, behavior: 'smooth', top: 90 }
    return { top: 0 }
  },
})

router.afterEach((to) => {
  document.title = (to.meta.title as string) ?? 'Cove'
})

export default router
