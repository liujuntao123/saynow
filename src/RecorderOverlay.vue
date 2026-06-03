<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import AppIcon from './components/AppIcon.vue';

const state = ref<'recording' | 'processing' | 'error'>('recording');
const elapsed = ref(0);
let startedAt = Date.now();
let timer: number | undefined;
let unlisten: (() => void) | undefined;

const label = computed(() => {
  if (state.value === 'processing') return '正在识别';
  if (state.value === 'error') return '识别失败';
  return '正在录音';
});

const elapsedText = computed(() => `${elapsed.value}s`);

onMounted(async () => {
  document.body.classList.add('recorder-body');
  timer = window.setInterval(() => {
    elapsed.value = Math.max(0, Math.floor((Date.now() - startedAt) / 1000));
  }, 250);
  unlisten = await getCurrentWindow().listen<{ state: 'recording' | 'processing' | 'error' }>('recorder-state', (event) => {
    state.value = event.payload.state;
    if (state.value === 'recording') {
      startedAt = Date.now();
      elapsed.value = 0;
    }
  });
});

onBeforeUnmount(() => {
  document.body.classList.remove('recorder-body');
  if (timer) window.clearInterval(timer);
  unlisten?.();
});
</script>

<template>
  <div class="recorder-overlay" :class="state">
    <span class="recorder-dot" aria-hidden="true"></span>
    <AppIcon :name="state === 'processing' ? 'activity' : 'mic'" />
    <strong>{{ label }}</strong>
    <span>{{ state === 'recording' ? elapsedText : '请稍候' }}</span>
  </div>
</template>
