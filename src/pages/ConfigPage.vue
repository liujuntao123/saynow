<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import UiPanel from '../components/UiPanel.vue';
import {
  DEFAULT_HOTKEY,
  HOTKEY_PRESETS,
  normalizeHotkeyPreset,
  type HotkeyPresetValue,
} from '../domain/hotkeyPresets';
import {
  DEFAULT_DOUBAO_ENDPOINT,
  DEFAULT_DOUBAO_RESOURCE_ID,
  DOUBAO_RESOURCE_OPTIONS,
  KIND_DASHSCOPE_MULTIMODAL,
  KIND_DOUBAO_WS,
  KIND_OPENAI_COMPATIBLE,
  QWEN_AUDIO_ASR_MODEL,
  isDashScopeMultimodalProvider,
  isDoubaoProvider,
  isProviderFormComplete,
  parseDoubaoExtra,
  providerSubtitle,
  serializeDoubaoExtra,
} from '../domain/providerKinds';
import { testProviderConnection } from '../api/tauri';
import { useToast } from '../composables/useToast';
import type { AppConfig, DoubaoAuthMode, PersonalizationPreferences, ProviderConfig } from '../types';

const props = defineProps<{
  config: AppConfig | null;
  providers: ProviderConfig[];
  preferences: PersonalizationPreferences;
  saving: boolean;
}>();

const emit = defineEmits<{
  save: [config: AppConfig];
  saveProvider: [provider: ProviderConfig];
  selectProvider: [id: number];
  deleteProvider: [id: number];
  updatePreferences: [preferences: PersonalizationPreferences];
}>();

const toast = useToast();
const emptyProvider = (): ProviderConfig => ({
  id: 0,
  provider: '',
  baseUrl: '',
  model: '',
  apiKeyRef: '',
  enabled: true,
  kind: KIND_OPENAI_COMPATIBLE,
  extraJson: '{}',
});
const providerForm = reactive<ProviderConfig>(emptyProvider());
const doubaoAuthMode = ref<DoubaoAuthMode>('legacy');
const doubaoAccessKey = ref('');
const showDoubaoAdvanced = ref(false);
const selectedProviderId = ref<number | null>(null);
const hotkey = ref<HotkeyPresetValue>(DEFAULT_HOTKEY);
const testingConnection = ref(false);
const connectionResult = ref<{ ok: boolean; message: string } | null>(null);

// 模板不再预填 credential-manager 引用（那只是环境变量别名，普通用户不可用），
// 让用户直接粘贴自己的 API Key。
const templates = [
  {
    id: 'mimo',
    label: 'MiMo',
    provider: 'MiMo',
    baseUrl: 'https://api.xiaomimimo.com/v1',
    model: 'mimo-v2.5',
    kind: KIND_OPENAI_COMPATIBLE,
  },
  {
    id: 'mimo-asr',
    label: 'MiMo ASR',
    provider: 'MiMo ASR',
    baseUrl: 'https://api.xiaomimimo.com/v1',
    model: 'mimo-v2.5-asr',
    kind: KIND_OPENAI_COMPATIBLE,
  },
  {
    id: 'qwen',
    label: 'Qwen',
    provider: 'Qwen',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    model: 'qwen3.5-omni-plus',
    kind: KIND_OPENAI_COMPATIBLE,
  },
  {
    id: 'qwen-audio-asr',
    label: 'Qwen Audio ASR',
    provider: 'Qwen Audio ASR',
    baseUrl: '',
    model: QWEN_AUDIO_ASR_MODEL,
    kind: KIND_DASHSCOPE_MULTIMODAL,
  },
  {
    id: 'doubao',
    label: '豆包 ASR',
    provider: 'Doubao ASR',
    baseUrl: DEFAULT_DOUBAO_ENDPOINT,
    model: DEFAULT_DOUBAO_RESOURCE_ID,
    kind: KIND_DOUBAO_WS,
  },
] as const;

const activeProvider = computed(() => props.providers.find((provider) => provider.enabled) ?? null);
const isDoubaoForm = computed(() => isDoubaoProvider(providerForm.kind, providerForm.provider));
const isDashScopeForm = computed(() =>
  isDashScopeMultimodalProvider(providerForm.kind, providerForm.model),
);
const providerFormComplete = computed(() =>
  isProviderFormComplete({
    ...providerForm,
    extraJson: isDoubaoForm.value
      ? serializeDoubaoExtra({
          authMode: doubaoAuthMode.value,
          accessKey: doubaoAccessKey.value,
        })
      : providerForm.extraJson ?? '{}',
  }),
);

watch(() => props.config, (config) => {
  if (!config) return;
  hotkey.value = normalizeHotkeyPreset(config.hotkey);
  if (!props.providers.length) {
    applyProviderToForm({ id: 0, ...config, enabled: true });
    selectedProviderId.value = null;
  }
}, { immediate: true });

watch(() => props.providers, (providers) => {
  const current = providers.find((provider) => provider.id === selectedProviderId.value);
  const next = current ?? providers.find((provider) => provider.enabled) ?? providers[0];
  if (next) {
    loadProvider(next);
  } else if (props.config) {
    applyProviderToForm({ id: 0, ...props.config, enabled: true });
    selectedProviderId.value = null;
  }
}, { immediate: true });

function applyProviderToForm(provider: ProviderConfig) {
  Object.assign(providerForm, emptyProvider(), provider, {
    kind: provider.kind || (
      isDoubaoProvider(provider.kind, provider.provider)
        ? KIND_DOUBAO_WS
        : isDashScopeMultimodalProvider(provider.kind, provider.model)
          ? KIND_DASHSCOPE_MULTIMODAL
          : KIND_OPENAI_COMPATIBLE
    ),
    extraJson: provider.extraJson || '{}',
  });
  const extra = parseDoubaoExtra(provider.extraJson);
  doubaoAuthMode.value = extra.authMode;
  doubaoAccessKey.value = extra.accessKey;
  showDoubaoAdvanced.value = false;
}

function loadProvider(provider: ProviderConfig) {
  applyProviderToForm(provider);
  selectedProviderId.value = provider.id;
  connectionResult.value = null;
}
function createProvider() {
  applyProviderToForm({ ...emptyProvider(), enabled: !props.providers.length });
  selectedProviderId.value = null;
  connectionResult.value = null;
}
function applyTemplate(templateId: string) {
  const template = templates.find((item) => item.id === templateId);
  if (!template) return;
  Object.assign(providerForm, {
    provider: template.provider,
    baseUrl: template.baseUrl,
    model: template.model,
    kind: template.kind,
    extraJson: template.kind === KIND_DOUBAO_WS
      ? serializeDoubaoExtra({ authMode: 'legacy', accessKey: '' })
      : '{}',
  });
  if (template.kind === KIND_DOUBAO_WS) {
    doubaoAuthMode.value = 'legacy';
    doubaoAccessKey.value = '';
    showDoubaoAdvanced.value = false;
  }
  connectionResult.value = null;
}
function isTemplateApplied(template: (typeof templates)[number]) {
  return providerForm.provider === template.provider
    && providerForm.model === template.model
    && (providerForm.kind || KIND_OPENAI_COMPATIBLE) === template.kind;
}

function buildProviderPayload(): ProviderConfig {
  const isDoubao = isDoubaoProvider(providerForm.kind, providerForm.provider);
  const isDashScope = isDashScopeMultimodalProvider(providerForm.kind, providerForm.model);
  return {
    ...providerForm,
    provider: providerForm.provider.trim(),
    baseUrl: isDoubao
      ? (providerForm.baseUrl.trim() || DEFAULT_DOUBAO_ENDPOINT)
      : providerForm.baseUrl.trim(),
    model: isDoubao
      ? (providerForm.model.trim() || DEFAULT_DOUBAO_RESOURCE_ID)
      : providerForm.model.trim(),
    apiKeyRef: providerForm.apiKeyRef.trim(),
    enabled: providerForm.enabled || !props.providers.length,
    kind: isDoubao
      ? KIND_DOUBAO_WS
      : isDashScope
        ? KIND_DASHSCOPE_MULTIMODAL
        : KIND_OPENAI_COMPATIBLE,
    extraJson: isDoubao
      ? serializeDoubaoExtra({
          authMode: doubaoAuthMode.value,
          accessKey: doubaoAccessKey.value,
        })
      : '{}',
  };
}

function saveProvider() {
  const provider = buildProviderPayload();
  if (!isProviderFormComplete(provider)) {
    toast.error(
      isDoubaoProvider(provider.kind, provider.provider)
        ? '请填写完整的豆包凭证（旧控制台需要 App Key + Access Key）。'
        : '请填写完整的供应商、模型、URL 和 API Key。',
    );
    return;
  }
  emit('saveProvider', provider);
}
async function testConnection() {
  if (!providerFormComplete.value) {
    toast.error('请先填写完整的供应商配置。');
    return;
  }
  testingConnection.value = true;
  connectionResult.value = null;
  try {
    const message = await testProviderConnection(buildProviderPayload());
    connectionResult.value = { ok: true, message };
  } catch (error) {
    connectionResult.value = { ok: false, message: typeof error === 'string' ? error : String(error) };
  } finally {
    testingConnection.value = false;
  }
}
function selectHotkey(value: HotkeyPresetValue) {
  hotkey.value = value;
}
function toggleCorrectionStrip() {
  emit('updatePreferences', {
    ...props.preferences,
    showCorrectionStrip: !props.preferences.showCorrectionStrip,
  });
}
function saveHotkey() {
  const provider = activeProvider.value;
  emit('save', {
    provider: provider?.provider ?? '',
    baseUrl: provider?.baseUrl ?? '',
    model: provider?.model ?? '',
    apiKeyRef: provider?.apiKeyRef ?? '',
    hotkey: normalizeHotkeyPreset(hotkey.value),
    kind: provider?.kind ?? KIND_OPENAI_COMPATIBLE,
    extraJson: provider?.extraJson ?? '{}',
  });
}
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
                <em>{{ providerSubtitle(provider) }}</em>
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

            <div v-if="isDoubaoForm" class="form-grid provider-grid config-form-card">
              <label class="field-span-2">
                供应商 <input v-model="providerForm.provider" placeholder="Doubao ASR" />
              </label>
              <label class="field-span-2">
                鉴权方式
                <select v-model="doubaoAuthMode">
                  <option value="legacy">旧控制台（App Key + Access Key）</option>
                  <option value="apiKey">新控制台（仅 API Key）</option>
                </select>
              </label>
              <label class="field-span-2">
                {{ doubaoAuthMode === 'apiKey' ? 'API Key' : 'App Key' }}
                <input
                  v-model="providerForm.apiKeyRef"
                  type="password"
                  :placeholder="doubaoAuthMode === 'apiKey' ? '粘贴 X-Api-Key' : '粘贴 App ID / App Key'"
                />
              </label>
              <label v-if="doubaoAuthMode === 'legacy'" class="field-span-2">
                Access Key
                <input v-model="doubaoAccessKey" type="password" placeholder="粘贴 Access Token" />
              </label>
              <label class="field-span-2">
                Resource Id
                <select v-model="providerForm.model">
                  <option
                    v-for="option in DOUBAO_RESOURCE_OPTIONS"
                    :key="option.id"
                    :value="option.value"
                  >
                    {{ option.label }}（{{ option.value }}）
                  </option>
                </select>
              </label>
              <div class="field-span-2 doubao-advanced">
                <button
                  class="ghost-button compact-action-button"
                  type="button"
                  @click="showDoubaoAdvanced = !showDoubaoAdvanced"
                >
                  {{ showDoubaoAdvanced ? '收起高级选项' : '高级选项（Endpoint）' }}
                </button>
                <label v-if="showDoubaoAdvanced">
                  WebSocket Endpoint
                  <input v-model="providerForm.baseUrl" :placeholder="DEFAULT_DOUBAO_ENDPOINT" />
                </label>
              </div>
            </div>

            <div v-else class="form-grid provider-grid config-form-card">
              <label>
                供应商 <input v-model="providerForm.provider" placeholder="输入名称" />
              </label>
              <label>
                模型 <input v-model="providerForm.model" placeholder="输入模型" />
              </label>
              <label class="field-span-2">
                URL
                <input
                  v-model="providerForm.baseUrl"
                  :placeholder="isDashScopeForm
                    ? 'https://你的WorkspaceId.cn-beijing.maas.aliyuncs.com/api/v1'
                    : 'https://api...'"
                />
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
          <div class="hotkey-picker">
            <button
              v-for="preset in HOTKEY_PRESETS"
              :key="preset.id"
              class="hotkey-option"
              :class="{ selected: hotkey === preset.value }"
              type="button"
              :disabled="saving"
              @click="selectHotkey(preset.value)"
            >
              <span class="hotkey-option-radio" aria-hidden="true"></span>
              <span class="hotkey-option-copy">
                <strong>{{ preset.label }}</strong>
                <em>{{ preset.description }}</em>
              </span>
            </button>
            <p class="hotkey-hint">按住所选快捷键开始录音，松开结束。</p>
            <div class="hotkey-actions">
              <button class="primary-button" type="button" :disabled="saving" @click="saveHotkey">
                <AppIcon name="save" /> 保存快捷键
              </button>
            </div>
          </div>
        </UiPanel>

        <UiPanel title="浮层显示" icon="text" class="overlay-panel custom-glass-panel">
          <button
            type="button"
            class="feature-toggle-row"
            :aria-pressed="preferences.showCorrectionStrip"
            @click="toggleCorrectionStrip"
          >
            <span class="feature-toggle-copy">
              <strong>显示文字编辑条</strong>
              <em>识别完成后显示“已输入”条，可编辑或撤销。</em>
            </span>
            <span class="ios-switch" :class="{ on: preferences.showCorrectionStrip }"></span>
          </button>
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
input, textarea, select { width: 100%; border: 1px solid #e1e9e6; border-radius: 12px; padding: 12px 14px; background: #f8fbfa; color: #182321; transition: all 0.3s; box-sizing: border-box; outline: none; font: inherit; }
input:focus, textarea:focus, select:focus { border-color: #0f8f83; background: #fff; box-shadow: 0 0 0 3px rgba(15, 143, 131, 0.1); }
label { display: flex; flex-direction: column; gap: 8px; font-size: 13px; font-weight: 600; color: #62706c; }
.doubao-advanced { display: flex; flex-direction: column; gap: 12px; align-items: flex-start; }
.doubao-advanced .ghost-button { align-self: flex-start; height: 34px; padding: 0 12px; }

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

/* 快捷键预设选择 */
.config-side-stack { display: flex; flex-direction: column; gap: 16px; min-height: 0; overflow: hidden; }
:deep(.hotkey-panel.custom-glass-panel) { padding: 18px; }
:deep(.hotkey-panel.custom-glass-panel .ui-panel-header) { padding-bottom: 14px; margin-bottom: 18px; }
:deep(.overlay-panel.custom-glass-panel) { padding: 18px; }
:deep(.overlay-panel.custom-glass-panel .ui-panel-header) { padding-bottom: 14px; margin-bottom: 18px; }
.hotkey-picker { display: flex; flex-direction: column; gap: 12px; }
.hotkey-option {
  width: 100%; display: flex; align-items: flex-start; justify-content: flex-start; gap: 12px; padding: 14px;
  border-radius: 14px; background: #f7faf9; border: 1px solid transparent; text-align: left;
}
.hotkey-option:hover:not(:disabled) { background: #eef6f4; }
.hotkey-option.selected {
  background: rgba(15, 143, 131, 0.08); border-color: rgba(15, 143, 131, 0.28);
}
.hotkey-option-radio {
  flex: 0 0 16px; width: 16px; height: 16px; margin-top: 2px; border-radius: 50%;
  border: 2px solid #b7c5c1; box-sizing: border-box;
}
.hotkey-option.selected .hotkey-option-radio {
  border-color: #0f8f83; box-shadow: inset 0 0 0 3px #0f8f83;
}
.hotkey-option-copy {
  display: flex; flex: 1 1 auto; flex-direction: column; align-items: flex-start; gap: 4px;
  min-width: 0; text-align: left;
}
.hotkey-option-copy strong { font-size: 14px; font-weight: 700; color: #1d1d1f; }
.hotkey-option-copy em { font-size: 12px; font-style: normal; color: #8a9793; line-height: 1.4; }
.hotkey-hint { margin: 0; color: #8a9793; font-size: 11px; line-height: 1.5; }
.hotkey-actions { display: flex; justify-content: flex-end; }
.hotkey-actions .primary-button { gap: 6px; }

/* 浮层显示开关 */
.feature-toggle-row {
  width: 100%;
  min-height: 72px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px;
  background: #fafafa;
  border: 1px solid #f0f0f5;
  border-radius: 14px;
  text-align: left;
}
.feature-toggle-row:hover {
  background: #fff;
  border-color: #0f8f83;
  box-shadow: 0 4px 16px rgba(15, 143, 131, 0.08);
}
.feature-toggle-row:focus-visible {
  border-color: #0f8f83;
  box-shadow: 0 0 0 3px rgba(15, 143, 131, 0.24);
}
.feature-toggle-copy {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.feature-toggle-copy strong {
  font-size: 13px;
  font-weight: 700;
  color: #1d1d1f;
  overflow-wrap: anywhere;
}
.feature-toggle-copy em {
  font-size: 12px;
  line-height: 1.45;
  color: #86868b;
  font-style: normal;
  overflow-wrap: anywhere;
}
.ios-switch {
  width: 44px;
  height: 24px;
  border-radius: 12px;
  background: #e5e5ea;
  position: relative;
  transition: background 0.3s;
  flex-shrink: 0;
  border: none;
  cursor: pointer;
  padding: 0;
}
.ios-switch::after {
  content: '';
  position: absolute;
  top: 2px;
  left: 2px;
  width: 20px;
  height: 20px;
  background: #fff;
  border-radius: 50%;
  transition: transform 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}
.ios-switch.on { background: #34c759; }
.ios-switch.on::after { transform: translateX(20px); }
button.ios-switch:focus-visible { box-shadow: 0 0 0 3px rgba(15, 143, 131, 0.24); }
</style>
