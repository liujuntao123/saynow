<script setup lang="ts">
import { onBeforeUnmount, reactive, ref, watch } from 'vue';
import AppIcon from '../components/AppIcon.vue';
import PageHeader from '../components/PageHeader.vue';
import UiPanel from '../components/UiPanel.vue';
import { formatHotkey } from '../domain/hotkeyRecorder';
import type { AppConfig } from '../types';

const props = defineProps<{
  config: AppConfig | null;
  saving: boolean;
}>();

const emit = defineEmits<{
  save: [config: AppConfig];
}>();

const form = reactive<AppConfig>({
  provider: 'MiMo',
  baseUrl: 'https://api.mimo-v2.com/v1',
  model: 'mimo-v2.5',
  apiKeyRef: 'credential-manager:mimo',
  hotkey: 'Ctrl+Space',
});
const recordingHotkey = ref(false);

watch(
  () => props.config,
  (config) => {
    if (config) Object.assign(form, config);
  },
  { immediate: true },
);

const templates = [
  { label: 'MiMo-v2.5', provider: 'MiMo', baseUrl: 'https://api.mimo-v2.com/v1', model: 'mimo-v2.5', apiKeyRef: 'credential-manager:mimo' },
  { label: 'Qwen3.5-Omni', provider: 'Qwen', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen3.5-omni', apiKeyRef: 'credential-manager:qwen' },
];

function applyTemplate(template: (typeof templates)[number]) {
  Object.assign(form, {
    provider: template.provider,
    baseUrl: template.baseUrl,
    model: template.model,
    apiKeyRef: template.apiKeyRef,
  });
}

function isSelectedTemplate(template: (typeof templates)[number]) {
  return form.provider === template.provider && form.baseUrl === template.baseUrl && form.model === template.model;
}

function recordHotkey(event: KeyboardEvent) {
  if (!recordingHotkey.value) return;
  event.preventDefault();
  event.stopPropagation();

  if (event.key === 'Escape') {
    recordingHotkey.value = false;
    return;
  }

  const hotkey = formatHotkey(event);
  if (!hotkey) return;

  form.hotkey = hotkey;
  recordingHotkey.value = false;
}

watch(recordingHotkey, (recording) => {
  if (recording) {
    window.addEventListener('keydown', recordHotkey, true);
  } else {
    window.removeEventListener('keydown', recordHotkey, true);
  }
}, { flush: 'sync' });

onBeforeUnmount(() => {
  window.removeEventListener('keydown', recordHotkey, true);
});
</script>

<template>
  <div class="page-stack">
    <PageHeader title="配置" icon="settings">
      <template #actions>
        <button class="primary-button icon-button" type="button" :disabled="saving" @click="emit('save', { ...form })">
          <AppIcon :name="saving ? 'activity' : 'save'" />
          {{ saving ? '保存中' : '保存配置' }}
        </button>
      </template>
    </PageHeader>

    <UiPanel title="供应商配置" icon="layers">
      <div class="template-row">
        <button
          v-for="template in templates"
          :key="template.label"
          class="template-button"
          :class="{ selected: isSelectedTemplate(template) }"
          type="button"
          @click="applyTemplate(template)"
        >
          <span class="template-radio" aria-hidden="true"></span>
          <strong>{{ template.label }}</strong>
          <span>{{ template.baseUrl }}</span>
        </button>
      </div>
      <div class="form-grid provider-grid">
        <label>
          供应商
          <input v-model="form.provider" />
        </label>
        <label>
          模型
          <input v-model="form.model" />
        </label>
        <label class="field-span-2">
          Base URL
          <input v-model="form.baseUrl" />
        </label>
        <label class="field-span-2">
          API Key 安全存储引用
          <input v-model="form.apiKeyRef" />
        </label>
      </div>
    </UiPanel>

    <UiPanel title="快捷键" icon="keyboard">
      <div class="hotkey-recorder" :class="{ recording: recordingHotkey }" tabindex="0" @keydown="recordHotkey">
        <span>{{ recordingHotkey ? '请按下快捷键组合' : form.hotkey || '未设置' }}</span>
        <div class="hotkey-actions">
          <button class="secondary-button icon-button" type="button" @click="recordingHotkey = true">
            <AppIcon name="keyboard" />
            录制
          </button>
          <button class="ghost-button icon-only-button" type="button" aria-label="清除快捷键" title="清除" @click="form.hotkey = ''">
            <AppIcon name="trash" />
          </button>
          <button v-if="recordingHotkey" class="ghost-button icon-only-button" type="button" aria-label="取消录制" title="取消" @click="recordingHotkey = false">
            <AppIcon name="x" />
          </button>
        </div>
      </div>
    </UiPanel>
  </div>
</template>
