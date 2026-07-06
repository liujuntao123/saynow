import { readonly, ref } from 'vue';

export interface ConfirmRequest {
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}

interface ActiveConfirm extends ConfirmRequest {
  resolve: (confirmed: boolean) => void;
}

const active = ref<ActiveConfirm | null>(null);

/** 弹出确认对话框，返回用户是否确认。同一时间只显示一个。 */
function confirm(request: ConfirmRequest): Promise<boolean> {
  active.value?.resolve(false);
  return new Promise<boolean>((resolve) => {
    active.value = { ...request, resolve };
  });
}

function settle(confirmed: boolean) {
  active.value?.resolve(confirmed);
  active.value = null;
}

export function useConfirm() {
  return {
    active: readonly(active),
    confirm,
    settle,
  };
}
