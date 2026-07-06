import { readonly, ref } from 'vue';

export type ToastKind = 'success' | 'error' | 'info';

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

const toasts = ref<ToastItem[]>([]);
let nextId = 1;

const TOAST_DURATION_MS: Record<ToastKind, number> = {
  success: 2600,
  info: 3200,
  error: 6000,
};
const MAX_TOASTS = 4;

function push(kind: ToastKind, message: string) {
  const text = message.trim();
  if (!text) return;
  const id = nextId++;
  toasts.value = [...toasts.value.slice(-(MAX_TOASTS - 1)), { id, kind, message: text }];
  window.setTimeout(() => dismiss(id), TOAST_DURATION_MS[kind]);
}

function dismiss(id: number) {
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

function formatError(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

export function useToast() {
  return {
    toasts: readonly(toasts),
    dismiss,
    success: (message: string) => push('success', message),
    info: (message: string) => push('info', message),
    error: (error: unknown, prefix?: string) => {
      const message = formatError(error);
      push('error', prefix ? `${prefix}：${message}` : message);
    },
  };
}
