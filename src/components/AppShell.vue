<script setup lang="ts">
defineProps<{
  activePage: string;
}>();

const emit = defineEmits<{
  navigate: [page: string];
}>();

const navItems = [
  { key: 'home', label: '首页', icon: 'home' },
  { key: 'config', label: '配置', icon: 'settings' },
  { key: 'data', label: '数据', icon: 'database' },
  { key: 'feedback', label: '反馈', icon: 'message' },
];

const iconPaths: Record<string, string[]> = {
  home: ['M3 10.5 12 3l9 7.5', 'M5 10v10h5v-6h4v6h5V10'],
  settings: [
    'M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z',
    'M19.4 15a1.7 1.7 0 0 0 .34 1.87l.04.04a2 2 0 0 1-2.83 2.83l-.04-.04a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 0 1-4 0v-.06a1.7 1.7 0 0 0-1-1.55 1.7 1.7 0 0 0-1.87.34l-.04.04a2 2 0 0 1-2.83-2.83l.04-.04A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 0 1 0-4h.06a1.7 1.7 0 0 0 1.55-1 1.7 1.7 0 0 0-.34-1.87l-.04-.04a2 2 0 0 1 2.83-2.83l.04.04A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.55V3a2 2 0 0 1 4 0v.06a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.04-.04a2 2 0 0 1 2.83 2.83l-.04.04A1.7 1.7 0 0 0 19.4 9c.25.6.85 1 1.55 1H21a2 2 0 0 1 0 4h-.06a1.7 1.7 0 0 0-1.55 1Z',
  ],
  database: ['M12 3c4.42 0 8 1.34 8 3s-3.58 3-8 3-8-1.34-8-3 3.58-3 8-3Z', 'M4 6v6c0 1.66 3.58 3 8 3s8-1.34 8-3V6', 'M4 12v6c0 1.66 3.58 3 8 3s8-1.34 8-3v-6'],
  message: ['M4 5h16v11H8l-4 4V5Z', 'M8 9h8', 'M8 13h5'],
};
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="sidebar-top">
        <div class="brand">
          <div class="brand-mark">语</div>
          <div class="brand-copy">
            <strong>说文</strong>
            <span>saynow</span>
          </div>
        </div>
      </div>
      <nav class="nav-list">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="nav-item"
          :class="{ active: activePage === item.key }"
          type="button"
          @click="emit('navigate', item.key)"
        >
          <span class="nav-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none">
              <path
                v-for="path in iconPaths[item.icon]"
                :key="path"
                :d="path"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </span>
          <em>{{ item.label }}</em>
        </button>
      </nav>
      <div class="sidebar-footer">
        <span class="pulse"></span>
        <em>后台监听已就绪</em>
      </div>
    </aside>
    <main class="main-panel">
      <slot />
    </main>
  </div>
</template>
