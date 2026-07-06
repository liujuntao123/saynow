<script setup lang="ts">
import AppIcon from './AppIcon.vue';
import { useToast } from '../composables/useToast';

const { toasts, dismiss } = useToast();

const iconByKind = {
  success: 'check',
  error: 'alert-circle',
  info: 'question',
} as const;
</script>

<template>
  <Teleport to="body">
    <TransitionGroup name="toast" tag="div" class="toast-stack" aria-live="polite">
      <div v-for="toast in toasts" :key="toast.id" class="toast-item" :class="toast.kind" role="status">
        <AppIcon :name="iconByKind[toast.kind]" class="toast-icon" />
        <span class="toast-message">{{ toast.message }}</span>
        <button type="button" class="toast-close" aria-label="关闭提示" @click="dismiss(toast.id)">
          <AppIcon name="x" />
        </button>
      </div>
    </TransitionGroup>
  </Teleport>
</template>

<style scoped>
.toast-stack {
  position: fixed;
  right: 24px;
  bottom: 24px;
  z-index: 1000;
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-width: min(420px, calc(100vw - 48px));
  pointer-events: none;
}

.toast-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 14px;
  border-radius: 12px;
  background: #1d1d1f;
  color: #fff;
  font-size: 13px;
  line-height: 1.5;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.24);
  pointer-events: auto;
}

.toast-item.success { background: #0f8f83; }
.toast-item.error { background: #c84d4d; }

.toast-icon { flex: 0 0 auto; width: 16px; height: 16px; margin-top: 2px; }
.toast-message { flex: 1 1 auto; min-width: 0; overflow-wrap: anywhere; white-space: pre-wrap; }

.toast-close {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.14);
  color: inherit;
  cursor: pointer;
  padding: 0;
}
.toast-close:hover { background: rgba(255, 255, 255, 0.26); }
.toast-close .app-icon { width: 12px; height: 12px; }

.toast-enter-active, .toast-leave-active { transition: all 0.24s ease; }
.toast-enter-from { opacity: 0; transform: translateY(12px); }
.toast-leave-to { opacity: 0; transform: translateX(24px); }
</style>
