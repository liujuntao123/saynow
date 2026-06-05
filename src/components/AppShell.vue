<script setup lang="ts">
import AppIcon from './AppIcon.vue';

defineProps<{
  activePage: string;
}>();

const emit = defineEmits<{
  navigate: [page: string];
}>();

const navItems = [
  { key: 'home', label: '首页', icon: 'home' },
  { key: 'config', label: '配置', icon: 'settings' },
  { key: 'personalization', label: '个性化', icon: 'spark' },
  { key: 'data', label: '数据', icon: 'database' },
  { key: 'feedback', label: '反馈', icon: 'message' },
];
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
            <AppIcon :name="item.icon" />
          </span>
          <em>{{ item.label }}</em>
        </button>
      </nav>
      <div class="sidebar-footer">
      </div>
    </aside>
    <main class="main-panel">
      <slot />
    </main>
  </div>
</template>
