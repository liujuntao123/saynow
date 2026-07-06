<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue';
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import UiPanel from '../components/UiPanel.vue';
import { formatHotkey, isModifierOnlyHotkey } from '../domain/hotkeyRecorder';
import { testProviderConnection } from '../api/tauri';
import { useToast } from '../composables/useToast';
import type { AppConfig, ProviderConfig } from '../types';

const props = defineProps<{
  config: AppConfig | null;
  providers: ProviderConfig[];
  saving: boolean;
}>();

const emit = defineEmits<{
  save: [config: AppConfig];
  saveProvider: [provider: ProviderConfig];
  selectProvider: [id: number];
  deleteProvider: [id: number];
  hotkeyRecordingChange: [recording: boolean];
}>();

const toast = useToast();
const providerForm = reactive<ProviderConfig>({
  id: 0, provider: '', baseUrl: '', model: '', apiKeyRef: '', enabled: true,
});
const selectedProviderId = ref<number | null>(null);
const hotkey = ref('Alt');
const recordingHotkey = ref(false);
const pendingModifierOnlyHotkey = ref<string | null>(null);
const testingConnection = ref(false);
const connectionResult = ref<{ ok: boolean; message: string } | null>(null);
let hotkeyCaptureTimer: ReturnType<typeof window.setTimeout> | null = null;
const HOTKEY_CAPTURE_TIMEOUT_MS = 15_000;

// 模板不再预填 credential-manager 引用（那只是环境变量别名，普通用户不可用），
// 让用户直接粘贴自己的 API Key。
const templates = [
  { id: 'mimo', label: 'MiMo', provider: 'MiMo', baseUrl: 'https://api.xiaomimimo.com/v1', model: 'mimo-v2.5' },
  { id: 'mimo-asr', label: 'MiMo ASR', provider: 'MiMo ASR', baseUrl: 'https://api.xiaomimimo.com/v1', model: 'mimo-v2.5-asr' },
  { id: 'qwen', label: 'Qwen', provider: 'Qwen', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen3.5-omni-plus' },
];

const activeProvider = computed(() => props.providers.find((provider) => provider.enabled) ?? null);
const providerFormComplete = computed(() =>
  Boolean(
    providerForm.provider.trim() &&
    providerForm.baseUrl.trim() &&
    providerForm.model.trim() &&
    providerForm.apiKeyRef.trim(),
  ),
);

watch(() => props.config, (config) => {
  if (!config) return;
  hotkey.value = config.hotkey;
  if (!props.providers.length) { Object.assign(providerForm, { id: 0, ...config, enabled: true }); selectedProviderId.value = null; }
}, { immediate: true });

watch(() => props.providers, (providers) => {
  const current = providers.find((provider) => provider.id === selectedProviderId.value);
  const next = current ?? providers.find((provider) => provider.enabled) ?? providers[0];
  if (next) { loadProvider(next); }
  else if (props.config) { Object.assign(providerForm, { id: 0, ...props.config, enabled: true }); selectedProviderId.value = null; }
}, { immediate: true });

function loadProvider(provider: ProviderConfig) {
  Object.assign(providerForm, provider);
  selectedProviderId.value = provider.id;
  connectionResult.value = null;
}
function createProvider() {
  Object.assign(providerForm, { id: 0, provider: '', baseUrl: '', model: '', apiKeyRef: '', enabled: !props.providers.length });
  selectedProviderId.value = null;
  connectionResult.value = null;
}
function applyTemplate(templateId: string) {
  const template = templates.find((item) => item.id === templateId);
  if (!template) return;
  Object.assign(providerForm, { provider: template.provider, baseUrl: template.baseUrl, model: template.model });
  connectionResult.value = null;
}
function isTemplateApplied(template: (typeof templates)[number]) { return providerForm.provider === template.provider && providerForm.model === template.model; }
function saveProvider() {
  const provider = { ...providerForm, provider: providerForm.provider.trim(), baseUrl: providerForm.baseUrl.trim(), model: providerForm.model.trim(), apiKeyRef: providerForm.apiKeyRef.trim(), enabled: providerForm.enabled || !props.providers.length };
  if (!provider.provider || !provider.baseUrl || !provider.model || !provider.apiKeyRef) {
    toast.error('请填写完整的供应商、模型、URL 和 API Key。');
    return;
  }
  emit('saveProvider', provider);
}
async function testConnection() {
  if (!providerFormComplete.value) {
    toast.error('请先填写完整的供应商、模型、URL 和 API Key。');
    return;
  }
  testingConnection.value = true;
  connectionResult.value = null;
  try {
    const message = await testProviderConnection({
      ...providerForm,
      provider: providerForm.provider.trim(),
      baseUrl: providerForm.baseUrl.trim(),
      model: providerForm.model.trim(),
      apiKeyRef: providerForm.apiKeyRef.trim(),
    });
    connectionResult.value = { ok: true, message };
  } catch (error) {
    connectionResult.value = { ok: false, message: typeof error === 'string' ? error : String(error) };
  } finally {
    testingConnection.value = false;
  }
}
function saveHotkey() {
  const provider = activeProvider.value;
  emit('save', {
    provider: provider?.provider ?? '',
    baseUrl: provider?.baseUrl ?? '',
    model: provider?.model ?? '',
    apiKeyRef: provider?.apiKeyRef ?? '',
    hotkey: hotkey.value,
  });
}
function recordHotkey(event: KeyboardEvent) {
  if (!recordingHotkey.value) return;
  event.preventDefault(); event.stopPropagation();
  if (event.key === 'Escape') { stopHotkeyCapture(); return; }
  const nextHotkey = formatHotkey(event);
  if (!nextHotkey) return;
  if (isModifierOnlyHotkey(nextHotkey)) { pendingModifierOnlyHotkey.value = nextHotkey; return; }
  hotkey.value = nextHotkey; pendingModifierOnlyHotkey.value = null; stopHotkeyCapture();
}
function finishModifierOnlyHotkey(event: KeyboardEvent) {
  if (!recordingHotkey.value || !pendingModifierOnlyHotkey.value) return;
  event.preventDefault(); event.stopPropagation();
  hotkey.value = pendingModifierOnlyHotkey.value; pendingModifierOnlyHotkey.value = null; stopHotkeyCapture();
}
function startHotkeyCapture() {
  recordingHotkey.value = true;
}
function stopHotkeyCapture() {
  recordingHotkey.value = false;
}
function handleCaptureReset() {
  if (recordingHotkey.value) stopHotkeyCapture();
}
function armHotkeyCaptureTimer() {
  clearHotkeyCaptureTimer();
  hotkeyCaptureTimer = window.setTimeout(stopHotkeyCapture, HOTKEY_CAPTURE_TIMEOUT_MS);
}
function clearHotkeyCaptureTimer() {
  if (!hotkeyCaptureTimer) return;
  window.clearTimeout(hotkeyCaptureTimer);
  hotkeyCaptureTimer = null;
}

watch(recordingHotkey, (recording) => {
  emit('hotkeyRecordingChange', recording);
  if (recording) {
    armHotkeyCaptureTimer();
    window.addEventListener('keydown', recordHotkey, true);
    window.addEventListener('keyup', finishModifierOnlyHotkey, true);
    window.addEventListener('blur', handleCaptureReset);
    document.addEventListener('visibilitychange', handleCaptureReset);
  }
  else {
    clearHotkeyCaptureTimer();
    window.removeEventListener('keydown', recordHotkey, true);
    window.removeEventListener('keyup', finishModifierOnlyHotkey, true);
    window.removeEventListener('blur', handleCaptureReset);
    document.removeEventListener('visibilitychange', handleCaptureReset);
    pendingModifierOnlyHotkey.value = null;
  }
}, { flush: 'sync' });

onBeforeUnmount(() => {
  stopHotkeyCapture();
  clearHotkeyCaptureTimer();
  window.removeEventListener('keydown', recordHotkey, true);
  window.removeEventListener('keyup', finishModifierOnlyHotkey, true);
  window.removeEventListener('blur', handleCaptureReset);
  document.removeEventListener('visibilitychange', handleCaptureReset);
  emit('hotkeyRecordingChange', false);
});
</script>

<template>
  <div class="page-stack">
    <PageHeader title="配置" icon="settings" />

    <section class="config-workspace">
      <UiPanel title="大模型供应商" :meta="`共 ${providers.length} 个`" icon="layers" class="provider-unified-panel custom-glass-panel">
        <template #headerActions>
          <button class="secondary-button icon-only-button provider-new-button" type="button" aria-label="新增供应商" title="新增" @click="createProvider">
            <AppIcon name="plus" />
          </button>
        </template>

        <div class="provider-unified-grid">
          <!-- 左侧列表 -->
          <div v-if="providers.length" class="provider-list">
            <button
              v-for="provider in providers"
              :key="provider.id"
              class="provider-list-item"
              :class="{ active: selectedProviderId === provider.id }"
              type="button"
              @click="loadProvider(provider)"
            >
              <span class="provider-card-copy">
                <span class="provider-card-top">
                  <strong>{{ provider.provider }}</strong>
                  <span
                    class="status-dot"
                    :class="{ disabled: !provider.enabled }"
                    :title="provider.enabled ? '当前使用中' : '未启用'"
                  ></span>
                </span>
                <em>{{ provider.model }}</em>
              </span>
            </button>
          </div>
          <div v-else class="provider-empty-actions">
            <EmptyState icon="layers" title="暂无供应商" description="添加供应商后可在这里切换使用。" />
            <button class="primary-button icon-button full-width" type="button" @click="createProvider">
              <AppIcon name="plus" /> 新增
            </button>
          </div>

          <!-- 右侧表单 -->
          <form class="provider-editor provider-editor-card" @submit.prevent="saveProvider">
            <div class="template-grid">
              <button
                v-for="template in templates"
                :key="template.id"
                class="template-button"
                :class="{ selected: isTemplateApplied(template) }"
                type="button"
                @click="applyTemplate(template.id)"
              >
                <span class="template-radio"></span>
                <strong>{{ template.label }}</strong>
              </button>
            </div>

            <div class="form-grid provider-grid config-form-card">
              <label>
                供应商 <input v-model="providerForm.provider" placeholder="输入名称" />
              </label>
              <label>
                模型 <input v-model="providerForm.model" placeholder="输入模型" />
              </label>
              <label class="field-span-2">
                URL <input v-model="providerForm.baseUrl" placeholder="https://api..." />
              </label>
              <label class="field-span-2">
                API Key <input v-model="providerForm.apiKeyRef" type="password" placeholder="粘贴您的 API Key" />
              </label>
            </div>

            <p
              v-if="connectionResult"
              class="connection-result"
              :class="connectionResult.ok ? 'ok' : 'failed'"
            >
              <AppIcon :name="connectionResult.ok ? 'check' : 'alert-circle'" />
              <span>{{ connectionResult.message }}</span>
            </p>

            <div class="provider-form-actions">
              <div class="item-actions">
                <button
                  class="secondary-button compact-action-button"
                  type="button"
                  :disabled="saving || testingConnection || !providerFormComplete"
                  @click="testConnection"
                >
                  <AppIcon name="activity" /> {{ testingConnection ? '测试中…' : '测试连接' }}
                </button>
                <button class="primary-button compact-action-button" type="submit" :disabled="saving">
                  <AppIcon name="save" /> 保存
                </button>
                <button
                  class="secondary-button compact-action-button"
                  type="button" :disabled="saving || !providerForm.id || providerForm.enabled" @click="emit('selectProvider', providerForm.id)"
                >
                  <AppIcon name="check" /> 启用
                </button>
                <button
                  class="danger-button compact-action-button icon-only-button"
                  type="button" :disabled="saving || !providerForm.id" @click="emit('deleteProvider', providerForm.id)" title="删除" aria-label="删除"
                >
                  <AppIcon name="trash" />
                </button>
              </div>
            </div>
          </form>
        </div>
      </UiPanel>

      <div class="config-side-stack">
        <UiPanel title="全局唤醒快捷键" icon="keyboard" class="hotkey-panel custom-glass-panel">
          <div class="hotkey-recorder" :class="{ recording: recordingHotkey }" tabindex="0" @keydown="recordHotkey">
            <div class="recorder-halo"></div>
            <span>{{ recordingHotkey ? (pendingModifierOnlyHotkey ? `${pendingModifierOnlyHotkey}（松开确认）` : '请按下按键...') : hotkey || '未设置（热键已禁用）' }}</span>
            <small v-if="recordingHotkey" class="hotkey-hint">按组合键设置；单独修饰键需按下后松开确认；Esc 取消</small>
            <div class="hotkey-actions">
              <button class="secondary-button icon-only-button" type="button" title="录制快捷键" aria-label="录制快捷键" @click="startHotkeyCapture"><AppIcon name="keyboard" /></button>
              <button class="ghost-button icon-only-button" type="button" title="清空（禁用热键）" aria-label="清空快捷键" @click="hotkey = ''"><AppIcon name="trash" /></button>
              <button v-if="recordingHotkey" class="ghost-button icon-only-button" type="button" title="取消录制" aria-label="取消录制" @click="stopHotkeyCapture"><AppIcon name="x" /></button>
              <button class="primary-button icon-only-button" type="button" :disabled="saving" title="保存快捷键" aria-label="保存快捷键" @click="saveHotkey"><AppIcon name="save" /></button>
            </div>
          </div>
        </UiPanel>
      </div>
    </section>
  </div>
</template>

<style scoped>
/* =========== 基础组件解耦注入 (替换原styles.css依赖) =========== */
.page-stack { display: flex; flex-direction: column; gap: 24px; min-height: 0; height: 100%; overflow: hidden; }
:deep(.custom-glass-panel) { background: #ffffff; border-radius: 20px; border: 1px solid rgba(0,0,0,0.03); box-shadow: 0 8px 32px -8px rgba(15,143,131,0.06); padding: 24px; }
:deep(.custom-glass-panel .ui-panel-header) { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid rgba(0,0,0,0.04); padding-bottom: 16px; margin-bottom: 24px; }
:deep(.provider-unified-panel.custom-glass-panel) { padding: 20px; }
:deep(.provider-unified-panel.custom-glass-panel .ui-panel-header) { padding-bottom: 14px; margin-bottom: 20px; }

/* 按钮全局优雅重写 */
button { cursor: pointer; transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1); font-family: inherit; display: inline-flex; align-items: center; justify-content: center; border: none; outline: none; }
button:disabled { opacity: 0.5; cursor: not-allowed; }
.primary-button { background: linear-gradient(135deg, #0f8f83, #08776f); color: white; border-radius: 12px; padding: 0 16px; height: 38px; font-weight: 600; box-shadow: 0 4px 12px rgba(15, 143, 131, 0.2); }
.primary-button:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 6px 16px rgba(15, 143, 131, 0.3); }
.secondary-button { background: #e5f4f1; color: #08776f; border-radius: 12px; padding: 0 16px; height: 38px; font-weight: 600; }
.secondary-button:hover:not(:disabled) { background: #d0ebe5; }
.ghost-button { background: transparent; color: #8a9793; padding: 0 16px; height: 38px; border-radius: 12px; }
.ghost-button:hover:not(:disabled) { color: #182321; background: rgba(0,0,0,0.04); }
.danger-button { background: #f8e6e4; color: #c84d4d; border-radius: 12px; padding: 0 16px; height: 38px; font-weight: 600; }
.danger-button:hover:not(:disabled) { background: #f0caca; }
.icon-only-button { width: 38px; border-radius: 50%; padding: 0; flex-shrink: 0; }
.full-width { width: 100%; gap: 8px; border-radius: 12px; }

/* 表单输入框全局优雅重写 */
input, textarea { width: 100%; border: 1px solid #e1e9e6; border-radius: 12px; padding: 12px 14px; background: #f8fbfa; color: #182321; transition: all 0.3s; box-sizing: border-box; outline: none; }
input:focus, textarea:focus { border-color: #0f8f83; background: #fff; box-shadow: 0 0 0 3px rgba(15, 143, 131, 0.1); }
label { display: flex; flex-direction: column; gap: 8px; font-size: 13px; font-weight: 600; color: #62706c; }

/* =========== 页面专属结构重塑 =========== */
.config-workspace { display: grid; grid-template-columns: minmax(0, 1fr) 240px; gap: 20px; align-items: stretch; flex: 1 1 auto; min-height: 0; overflow: hidden; }
@media (max-width: 900px) { .config-workspace { grid-template-columns: 1fr; } }
.provider-unified-grid { display: grid; grid-template-columns: 200px minmax(0, 1fr); gap: 24px; flex: 1 1 auto; min-height: 0; overflow: hidden; }
@media (max-width: 768px) { .provider-unified-grid { grid-template-columns: 1fr; } }

/* 导航栏 */
.provider-list { display: flex; flex-direction: column; gap: 8px; min-height: 0; overflow-y: auto; border-right: 1px dashed rgba(0,0,0,0.08); padding-right: 14px; scrollbar-gutter: stable; }
.provider-list-item {
  width: 100%; display: flex; align-items: center; justify-content: space-between; padding: 14px 14px;
  background: transparent; border-radius: 12px; text-align: left; border: none; transition: all 0.2s;
}
.provider-list-item:hover { background: rgba(0,0,0,0.02); }
.provider-list-item.active { background: rgba(15,143,131,0.06); }
.provider-card-copy { display: flex; flex-direction: column; gap: 4px; width: 100%; }
.provider-card-top { display: flex; justify-content: space-between; align-items: center; gap: 10px; }
.provider-list-item strong { font-weight: 600; font-size: 14px; color: #1d1d1f; }
.provider-list-item em { font-size: 12px; color: #86868b; font-style: normal; }
.status-dot { display: block; flex: 0 0 9px; width: 9px; height: 9px; min-width: 9px; min-height: 9px; border-radius: 50%; background: var(--accent, #0f8f83); padding: 0; margin: 0; box-shadow: 0 0 6px rgba(15,143,131,0.4); }
.status-dot.disabled { background: #d1d1d6; box-shadow: none; }

/* 模板按钮 */
.provider-editor { display: flex; flex-direction: column; min-height: 0; overflow: hidden; }
.template-grid { display: flex; flex-wrap: wrap; gap: 10px; margin-bottom: 18px; }
.template-button {
  display: inline-flex; align-items: center; gap: 8px; min-width: 0; padding: 10px 16px; border-radius: 20px;
  background: #f5f5f7; border: 1px solid transparent; color: #555; font-size: 13px; font-weight: 500;
}
.template-button strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.template-button:hover { background: #ebebeb; }
.template-button.selected { background: rgba(15,143,131,0.08); border-color: rgba(15,143,131,0.2); color: #0f8f83; }
.template-radio { display: none; }

/* 表单布局 */
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: auto; padding: 0; }
.field-span-2 { grid-column: 1 / -1; }
.connection-result {
  display: inline-flex; align-items: flex-start; gap: 8px; margin: 14px 0 0;
  padding: 10px 14px; border-radius: 10px; font-size: 13px; font-weight: 500; line-height: 1.5;
  overflow-wrap: anywhere;
}
.connection-result .app-icon { flex: 0 0 auto; width: 15px; height: 15px; margin-top: 2px; }
.connection-result.ok { background: #e6f3e9; color: #247348; }
.connection-result.failed { background: #f8e6e4; color: #c84d4d; }
.provider-form-actions { margin-top: 24px; padding-top: 20px; border-top: 1px solid rgba(0,0,0,0.04); display: flex; justify-content: flex-end; align-items: center; }
.item-actions { display: flex; gap: 12px; }
.compact-action-button { gap: 6px; }
.compact-action-button .app-icon { width: 15px; height: 15px; }

/* 快捷键艺术录制区 */
.config-side-stack { display: flex; flex-direction: column; gap: 16px; min-height: 0; overflow: hidden; }
:deep(.hotkey-panel.custom-glass-panel) { padding: 18px; }
:deep(.hotkey-panel.custom-glass-panel .ui-panel-header) { padding-bottom: 14px; margin-bottom: 18px; }
.hotkey-recorder {
  position: relative; height: 156px; border-radius: 14px; overflow: hidden;
  background: linear-gradient(180deg, #fafafa 0%, #f0f0f0 100%); border: 1px solid rgba(0,0,0,0.05);
  display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 18px; transition: all 0.4s; outline: none;
}
.recorder-halo { position: absolute; top: 50%; left: 50%; width: 110px; height: 110px; background: #0f8f83; filter: blur(42px); opacity: 0; transform: translate(-50%, -50%); transition: opacity 0.5s; pointer-events: none; }
.hotkey-recorder.recording { box-shadow: inset 0 0 0 2px #0f8f83; }
.hotkey-recorder.recording .recorder-halo { opacity: 0.15; animation: pulseHalo 2s infinite; }
.hotkey-recorder span { position: relative; z-index: 1; font-size: 20px; font-weight: 700; color: #1d1d1f; letter-spacing: 0; text-align: center; padding: 0 12px; }
.hotkey-hint { position: relative; z-index: 1; margin: -10px 12px 0; color: #8a9793; font-size: 11px; line-height: 1.5; text-align: center; }
.hotkey-actions { position: relative; z-index: 1; display: flex; gap: 8px; }
.hotkey-actions .icon-only-button { width: 34px; height: 34px; }
@keyframes pulseHalo { 0% { transform: translate(-50%, -50%) scale(1); } 50% { transform: translate(-50%, -50%) scale(1.2); } 100% { transform: translate(-50%, -50%) scale(1); } }
</style>
