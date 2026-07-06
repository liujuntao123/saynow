<script setup lang="ts">
import { useConfirm } from '../composables/useConfirm';

const { active, settle } = useConfirm();

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') settle(false);
  if (event.key === 'Enter') settle(true);
}
</script>

<template>
  <Teleport to="body">
    <Transition name="confirm-fade">
      <div
        v-if="active"
        class="confirm-backdrop"
        role="dialog"
        aria-modal="true"
        :aria-label="active.title"
        tabindex="-1"
        @keydown="handleKeydown"
        @click.self="settle(false)"
      >
        <div class="confirm-card">
          <h3>{{ active.title }}</h3>
          <p v-if="active.description">{{ active.description }}</p>
          <div class="confirm-actions">
            <button type="button" class="confirm-cancel" @click="settle(false)">
              {{ active.cancelLabel ?? '取消' }}
            </button>
            <button
              type="button"
              class="confirm-ok"
              :class="{ danger: active.danger }"
              autofocus
              @click="settle(true)"
            >
              {{ active.confirmLabel ?? '确认' }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.confirm-backdrop {
  position: fixed;
  inset: 0;
  z-index: 999;
  display: grid;
  place-items: center;
  background: rgba(15, 23, 42, 0.32);
  backdrop-filter: blur(2px);
}

.confirm-card {
  width: min(400px, calc(100vw - 48px));
  border-radius: 16px;
  background: #ffffff;
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.24);
  padding: 22px 24px 18px;
}

.confirm-card h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: #1d1d1f;
}

.confirm-card p {
  margin: 10px 0 0;
  font-size: 13px;
  line-height: 1.6;
  color: #62706c;
  overflow-wrap: anywhere;
}

.confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;
}

.confirm-actions button {
  border: none;
  border-radius: 10px;
  min-height: 36px;
  padding: 0 18px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.2s ease, transform 0.2s ease;
}

.confirm-cancel {
  background: #f1f5f4;
  color: #62706c;
}
.confirm-cancel:hover { background: #e5ebe9; }

.confirm-ok {
  background: #0f8f83;
  color: #fff;
}
.confirm-ok:hover { background: #0c7c72; }
.confirm-ok.danger { background: #c84d4d; }
.confirm-ok.danger:hover { background: #b13f3f; }

.confirm-fade-enter-active, .confirm-fade-leave-active { transition: opacity 0.18s ease; }
.confirm-fade-enter-from, .confirm-fade-leave-to { opacity: 0; }
</style>
