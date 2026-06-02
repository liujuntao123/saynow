<script setup lang="ts">
defineProps<{
  activePage: string;
}>();

const emit = defineEmits<{
  navigate: [page: string];
}>();

const navItems = [
  { key: 'home', label: '首页', icon: '⌂' },
  { key: 'config', label: '配置', icon: '⚙' },
  { key: 'data', label: '数据', icon: '▦' },
  { key: 'feedback', label: '反馈', icon: '↗' },
];
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">语</div>
        <div>
          <strong>语音输入助手</strong>
          <span>Voice Assistant</span>
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
          <span>{{ item.icon }}</span>
          {{ item.label }}
        </button>
      </nav>
      <div class="sidebar-footer">
        <span class="pulse"></span>
        后台监听已就绪
      </div>
    </aside>
    <main class="main-panel">
      <slot />
    </main>
  </div>
</template>
