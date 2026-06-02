<script setup lang="ts">
import { reactive, watch } from 'vue';
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
</script>

<template>
  <div class="page-stack">
    <header class="page-header">
      <div>
        <h1>配置</h1>
        <p>配置模型供应商、模型 ID、API Key 安全存储引用和语音识别快捷键。</p>
      </div>
      <button class="primary-button" type="button" :disabled="saving" @click="emit('save', { ...form })">
        {{ saving ? '保存中' : '保存配置' }}
      </button>
    </header>

    <section class="content-section">
      <div class="section-title">
        <h2>模型模板</h2>
      </div>
      <div class="template-row">
        <button v-for="template in templates" :key="template.label" class="template-button" type="button" @click="applyTemplate(template)">
          <strong>{{ template.label }}</strong>
          <span>{{ template.baseUrl }}</span>
        </button>
      </div>
    </section>

    <section class="form-grid">
      <label>
        供应商
        <input v-model="form.provider" />
      </label>
      <label>
        Base URL
        <input v-model="form.baseUrl" />
      </label>
      <label>
        模型
        <input v-model="form.model" />
      </label>
      <label>
        API Key 安全存储引用
        <input v-model="form.apiKeyRef" />
      </label>
      <label>
        语音识别快捷键
        <input v-model="form.hotkey" />
      </label>
    </section>
  </div>
</template>
