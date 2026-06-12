<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import UiPanel from '../components/UiPanel.vue';
import type { CorrectionRecord, LearningEngineConfig, LearningRule, PersonalizationPreferences, StylePrompt, VocabularyItem } from '../types';

const props = defineProps<{
  vocabulary: VocabularyItem[];
  styles: StylePrompt[];
  preferences: PersonalizationPreferences;
  learningEngineConfig: LearningEngineConfig;
  learningRules: LearningRule[];
  correctionRecords: CorrectionRecord[];
  saving: boolean;
}>();
const emit = defineEmits<{
  addVocabularyTerms: [terms: string[]];
  deleteVocabulary: [id: number];
  addStyle: [item: StylePrompt];
  updateStyle: [item: StylePrompt];
  deleteStyle: [id: number];
  updatePreferences: [preferences: PersonalizationPreferences];
  updateLearningEngine: [config: LearningEngineConfig];
  runLearningEngine: [];
}>();

const activeTab = ref<'vocabulary' | 'styles' | 'features'>('vocabulary');
const vocabularyText = ref('');
const styleForm = reactive<StylePrompt>({ id: 0, name: '', prompt: '', enabled: true });
const selectedStyleId = ref<number | null>(null);
const preferencesForm = reactive<PersonalizationPreferences>({ removeTrailingPeriod: false });
const learningEngineForm = reactive<LearningEngineConfig>({
  enabled: false,
  provider: '',
  baseUrl: '',
  model: '',
  apiKeyRef: '',
  runMode: 'llmAssist',
  minNewCorrections: 5,
  idleSeconds: 30,
});
watch(() => props.styles, (items) => {
  const current = items.find((item) => item.id === selectedStyleId.value);
  const next = current ?? items.find((item) => item.enabled) ?? items[0];
  selectedStyleId.value = next?.id ?? null; Object.assign(styleForm, next ?? { id: 0, name: '', prompt: '', enabled: true });
}, { immediate: true });
watch(() => props.preferences, (preferences) => {
  Object.assign(preferencesForm, preferences);
}, { immediate: true });
watch(() => props.learningEngineConfig, (config) => {
  Object.assign(learningEngineForm, config);
}, { immediate: true });

function selectStyle(item: StylePrompt) { selectedStyleId.value = item.id; Object.assign(styleForm, item); }
function submitVocabulary() {
  const terms = vocabularyText.value.split('\n').map((term) => term.trim()).filter(Boolean);
  if (!terms.length) return; emit('addVocabularyTerms', terms); vocabularyText.value = '';
}
function submitStyle() {
  if (!styleForm.name.trim() || !styleForm.prompt.trim()) return;
  const payload = { ...styleForm, name: styleForm.name.trim(), prompt: styleForm.prompt.trim() };
  if (styleForm.id) { emit('updateStyle', payload); return; } else { emit('addStyle', payload); }
  Object.assign(styleForm, { id: 0, name: '', prompt: '', enabled: true }); selectedStyleId.value = null;
}
function createStyleDraft() { selectedStyleId.value = null; Object.assign(styleForm, { id: 0, name: '', prompt: '', enabled: true }); }
function updateRemoveTrailingPeriod(enabled: boolean) {
  preferencesForm.removeTrailingPeriod = enabled;
  emit('updatePreferences', { ...preferencesForm });
}
function saveLearningEngine() {
  emit('updateLearningEngine', {
    enabled: learningEngineForm.enabled,
    provider: learningEngineForm.provider.trim(),
    baseUrl: learningEngineForm.baseUrl.trim(),
    model: learningEngineForm.model.trim(),
    apiKeyRef: learningEngineForm.apiKeyRef.trim(),
    runMode: 'llmAssist',
    minNewCorrections: Number(learningEngineForm.minNewCorrections) || 5,
    idleSeconds: Number(learningEngineForm.idleSeconds) || 30,
  });
}
function learningRuleStatusText(status: string) {
  const labels: Record<string, string> = {
    candidate: '候选规则',
    active: '已采用',
    pinned: '已固定',
    rejected: '已忽略',
  };
  return labels[status] ?? '待确认';
}
function learningRuleRiskText(risk: string) {
  const labels: Record<string, string> = {
    low: '低风险',
    medium: '中风险',
    high: '高风险',
  };
  return labels[risk] ?? '风险未知';
}
function learningRuleTip(rule: LearningRule) {
  return `${learningRuleStatusText(rule.status)}：这条规则由学习引擎从纠错记录中整理得出。${learningRuleRiskText(rule.risk)}：表示自动放入识别提示词时的误伤风险。`;
}
</script>

<template>
  <div class="page-stack">
    <PageHeader title="个性化" icon="spark" />

    <div class="tabs segmented-control">
      <div class="segment-pill" :class="activeTab"></div>
      <button :class="{ active: activeTab === 'vocabulary' }" type="button" @click="activeTab = 'vocabulary'">
        <AppIcon name="book" /> 自定义词库
      </button>
      <button :class="{ active: activeTab === 'styles' }" type="button" @click="activeTab = 'styles'">
        <AppIcon name="spark" /> 风格提示词
      </button>
      <button :class="{ active: activeTab === 'features' }" type="button" @click="activeTab = 'features'">
        <AppIcon name="settings" /> 功能配置
      </button>
    </div>

    <section v-if="activeTab === 'vocabulary'" class="split-section art-board">
      <form class="editor-panel art-card" @submit.prevent="submitVocabulary">
        <div class="panel-title-block panel-title-with-action art-hero">
          <span class="panel-title-icon art-icon-box"><AppIcon name="book" /></span>
          <h2>汇入新词条</h2>
          <button class="primary-button icon-only-button ml-auto" type="submit" aria-label="添加" title="添加"><AppIcon name="plus" /></button>
        </div>
        <div class="canvas-wrap">
          <textarea v-model="vocabularyText" rows="11" placeholder="输入特定领域的专有名词或表达，每行一个。AI 将更容易准确识别它们。" class="canvas-textarea"></textarea>
        </div>
      </form>

      <UiPanel class="list-panel art-card clean-panel" title="专属词库" :meta="`收录 ${vocabulary.length} 词`" icon="database">
        <div v-if="vocabulary.length" class="vocabulary-tabs bubble-cloud">
          <div v-for="item in vocabulary" :key="item.id" class="vocabulary-tab bubble-tag" :class="{ disabled: !item.enabled }">
            <strong>{{ item.term }}</strong>
            <button type="button" class="bubble-close" aria-label="删除" title="删除" @click="emit('deleteVocabulary', item.id)">
              <AppIcon name="x" />
            </button>
          </div>
        </div>
        <EmptyState v-else icon="book" title="空空如也" description="添加专属词汇，提升识别精准度。" />
      </UiPanel>
    </section>

    <section v-if="activeTab === 'styles'" class="style-management art-board reverse-on-mobile">
      <UiPanel class="style-list-panel art-card clean-panel" title="提示词" :meta="`${styles.length}`" icon="spark">
        <template #headerActions>
          <button class="secondary-button icon-only-button" type="button" @click="createStyleDraft"><AppIcon name="plus" /></button>
        </template>
        <div v-if="styles.length" class="managed-list style-list">
          <button
            v-for="item in styles"
            :key="item.id"
            class="managed-list-item style-item"
            :class="{ active: selectedStyleId === item.id }"
            type="button" @click="selectStyle(item)"
          >
            <span class="style-info">
              <strong>{{ item.name }}</strong>
              <em>{{ item.prompt }}</em>
            </span>
            <div class="ios-switch" :class="{ on: item.enabled }"></div>
          </button>
        </div>
        <EmptyState v-else icon="spark" title="暂无提示词" description="用自然语言定义你想要的整理风格。" />
      </UiPanel>

      <form class="editor-panel style-editor-panel art-card" @submit.prevent="submitStyle">
        <div class="panel-title-block art-hero">
          <span class="panel-title-icon art-icon-box" :class="{ 'editing': styleForm.id }"><AppIcon :name="styleForm.id ? 'settings' : 'spark'" /></span>
          <h2>{{ styleForm.id ? '调优提示词' : '构思新提示词' }}</h2>
        </div>
        <div class="form-row">
          <label>风格命名<input v-model="styleForm.name" placeholder="例如：正式会议纪要..." /></label>
        </div>
        <div class="form-row">
          <label>指令要求
            <div class="canvas-wrap">
              <textarea v-model="styleForm.prompt" rows="4" class="canvas-textarea style-prompt-textarea" placeholder="描述你希望 AI 如何润色和整理文本..."></textarea>
            </div>
          </label>
        </div>
        <div class="provider-form-actions form-footer">
          <label class="check-line"><input v-model="styleForm.enabled" type="checkbox" /> 启用此风格</label>
          <div class="item-actions">
            <button class="primary-button compact-action-button icon-only-button" type="submit" title="保存"><AppIcon name="save" /></button>
            <button
              v-if="styleForm.id" class="danger-button icon-only-button"
              type="button" title="删除" @click="emit('deleteStyle', styleForm.id)"
            ><AppIcon name="trash" /></button>
          </div>
        </div>
      </form>
    </section>

    <section v-if="activeTab === 'features'" class="feature-section art-board feature-grid">
      <UiPanel class="learning-panel art-card clean-panel" title="学习引擎" meta="LLM 整理纠错" icon="spark">
        <form class="learning-engine-form" @submit.prevent="saveLearningEngine">
          <button
            type="button"
            class="feature-toggle-row learning-enable-row"
            :aria-pressed="learningEngineForm.enabled"
            @click="learningEngineForm.enabled = !learningEngineForm.enabled"
          >
            <span class="feature-toggle-copy">
              <strong>启用 LLM 学习层</strong>
              <em>用独立文本模型整理纠错记录，生成可审阅的个性化规则。</em>
            </span>
            <span class="ios-switch" :class="{ on: learningEngineForm.enabled }"></span>
          </button>

          <div class="form-grid learning-form-grid">
            <label>
              供应商 <input v-model="learningEngineForm.provider" placeholder="OpenAI / Qwen / MiMo" />
            </label>
            <label>
              模型 <input v-model="learningEngineForm.model" placeholder="gpt-4.1-mini" />
            </label>
            <label class="field-span-2">
              URL <input v-model="learningEngineForm.baseUrl" placeholder="https://api..." />
            </label>
            <label class="field-span-2">
              API Key <input v-model="learningEngineForm.apiKeyRef" type="password" placeholder="credential-manager:learning" />
            </label>
            <label>
              触发条数 <input v-model.number="learningEngineForm.minNewCorrections" type="number" min="1" max="100" />
            </label>
            <label>
              空闲秒数 <input v-model.number="learningEngineForm.idleSeconds" type="number" min="5" max="3600" />
            </label>
          </div>

          <div class="provider-form-actions form-footer">
            <span class="feature-toggle-copy learning-status">
              <strong>{{ learningEngineForm.enabled ? '已启用' : '未启用' }}</strong>
              <em>{{ learningEngineForm.enabled ? '保存后，后续整理任务将使用此模型。' : '关闭时只保存纠错记录，不请求学习模型。' }}</em>
            </span>
            <button class="primary-button compact-action-button icon-only-button" type="submit" :disabled="saving" title="保存学习引擎">
              <AppIcon :name="saving ? 'activity' : 'save'" />
            </button>
            <button class="secondary-button compact-action-button icon-only-button" type="button" :disabled="saving" title="立即整理" @click="emit('runLearningEngine')">
              <AppIcon name="activity" />
            </button>
          </div>
        </form>
      </UiPanel>

      <UiPanel class="feature-panel art-card clean-panel" title="结果处理" meta="识别后规则" icon="settings">
        <div class="feature-list">
          <button
            type="button"
            class="feature-toggle-row"
            :aria-pressed="preferencesForm.removeTrailingPeriod"
            @click="updateRemoveTrailingPeriod(!preferencesForm.removeTrailingPeriod)"
          >
            <span class="feature-toggle-copy">
              <strong>去除尾句号</strong>
              <em>识别文本以句号结尾时，自动去掉最后一个句号。</em>
            </span>
            <span class="ios-switch" :class="{ on: preferencesForm.removeTrailingPeriod }"></span>
          </button>
        </div>
      </UiPanel>

      <UiPanel class="rules-panel art-card clean-panel" title="学习规则" :meta="`${learningRules.length} 条`" icon="database">
        <div v-if="learningRules.length" class="learning-rule-list">
          <div v-for="rule in learningRules" :key="rule.id" class="learning-rule-item">
            <div class="learning-rule-head">
              <strong>{{ rule.description }}</strong>
              <span class="rule-badge-wrap">
                <span class="rule-badge" :class="[rule.status, rule.risk]">
                  {{ learningRuleStatusText(rule.status) }} · {{ learningRuleRiskText(rule.risk) }}
                </span>
                <span class="rule-help" tabindex="0" :aria-label="learningRuleTip(rule)">
                  <AppIcon name="question" />
                  <span class="rule-tip" role="tooltip">{{ learningRuleTip(rule) }}</span>
                </span>
              </span>
            </div>
            <p v-if="rule.matchHints || rule.fromText || rule.toText">
              <span v-if="rule.matchHints">上下文：{{ rule.matchHints }}</span>
              <span v-if="rule.fromText || rule.toText">倾向：{{ rule.fromText || '∅' }} -> {{ rule.toText || '∅' }}</span>
            </p>
          </div>
        </div>
        <EmptyState v-else icon="database" title="暂无学习规则" description="保存纠错后，可由学习引擎整理为候选规则。" />
      </UiPanel>

      <UiPanel class="corrections-panel art-card clean-panel" title="纠错记录" :meta="`${correctionRecords.length} 条`" icon="fileText">
        <div v-if="correctionRecords.length" class="correction-record-list">
          <div v-for="record in correctionRecords" :key="record.id" class="correction-record-item">
            <div class="correction-record-head">
              <strong>#{{ record.id }}</strong>
              <span class="rule-badge" :class="{ candidate: record.learningProcessedAt, high: record.errorMessage }">
                {{ record.learningProcessedAt ? '已学习' : '待学习' }}
              </span>
            </div>
            <div class="correction-diff">
              <p><span>识别</span>{{ record.rawText }}</p>
              <p><span>修正</span>{{ record.correctedText }}</p>
            </div>
            <div class="correction-record-meta">
              <span>{{ record.applied ? '已替换目标文本' : '未替换目标文本' }}</span>
              <span v-if="record.errorMessage">{{ record.errorMessage }}</span>
            </div>
          </div>
        </div>
        <EmptyState v-else icon="fileText" title="暂无纠错记录" description="通过纠错浮窗确认修改后，记录会显示在这里。" />
      </UiPanel>
    </section>
  </div>
</template>

<style scoped>
/* 按钮与输入框基础重写 */
button { cursor: pointer; transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1); font-family: inherit; display: inline-flex; align-items: center; justify-content: center; border: none; outline: none; }
button:disabled { opacity: 0.5; cursor: not-allowed; }
.primary-button { background: linear-gradient(135deg, #0f8f83, #08776f); color: white; border-radius: 12px; padding: 0 16px; height: 38px; font-weight: 600; box-shadow: 0 4px 12px rgba(15, 143, 131, 0.2); }
.primary-button:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 6px 16px rgba(15, 143, 131, 0.3); }
.secondary-button { background: #e5f4f1; color: #08776f; border-radius: 12px; padding: 0 16px; height: 38px; font-weight: 600; }
.secondary-button:hover:not(:disabled) { background: #d0ebe5; }
.danger-button { background: #f8e6e4; color: #c84d4d; border-radius: 12px; padding: 0 16px; height: 38px; font-weight: 600; }
.danger-button:hover:not(:disabled) { background: #f0caca; }
.icon-only-button { width: 38px; border-radius: 50%; padding: 0; flex-shrink: 0; }
input, textarea { width: 100%; border: 1px solid #e1e9e6; border-radius: 12px; padding: 12px 14px; background: #f8fbfa; color: #182321; transition: all 0.3s; box-sizing: border-box; outline: none; font-family: inherit; }
input:focus, textarea:focus { border-color: #0f8f83; background: #fff; box-shadow: 0 0 0 3px rgba(15, 143, 131, 0.1); }
label { display: flex; flex-direction: column; gap: 8px; font-size: 13px; font-weight: 600; color: #62706c; }
.check-line { flex-direction: row; align-items: center; cursor: pointer; }
.check-line input { width: 18px; height: 18px; accent-color: #0f8f83; margin: 0; cursor: pointer; }

/* 页面排版 */
.page-stack { display: flex; flex-direction: column; gap: 20px; min-height: 0; height: 100%; overflow: hidden; align-items: stretch; }
.art-board { width: 100%; display: grid; grid-template-columns: 1fr 1fr; gap: 20px; align-items: stretch; flex: 1 1 auto; min-height: 0; overflow: hidden; animation: fadeUp 0.4s ease; }
.split-section { grid-template-columns: minmax(280px, 0.72fr) minmax(0, 1.28fr); }
.style-management { grid-template-columns: minmax(260px, 0.66fr) minmax(0, 1.34fr); }
.single-column-board { grid-template-columns: minmax(0, 1fr); align-items: start; }
.feature-grid {
  grid-template-columns: minmax(340px, 0.82fr) minmax(420px, 1.18fr);
  grid-auto-rows: min-content;
  align-items: start;
  overflow-y: auto;
  padding-right: 4px;
  scrollbar-gutter: stable;
}
@media (max-width: 900px) {
  .art-board { grid-template-columns: 1fr; }
  .feature-grid { grid-template-columns: minmax(0, 1fr); padding-right: 0; }
  .reverse-on-mobile { display: flex; flex-direction: column-reverse; }
}

/* 分段控制器覆盖原本 tabs */
.tabs.segmented-control {
  position: relative; display: inline-flex; align-self: flex-start; background: #f0f0f5; padding: 4px;
  border-radius: 14px; border: none; box-shadow: inset 0 2px 4px rgba(0,0,0,0.04); margin: -6px 0 0; flex: 0 0 auto;
}
.segment-pill {
  position: absolute; top: 4px; bottom: 4px; width: calc(33.333% - 4px); background: #ffffff;
  border-radius: 10px; box-shadow: 0 2px 8px rgba(0,0,0,0.08); transition: transform 0.4s cubic-bezier(0.25, 0.8, 0.25, 1);
}
.segment-pill.vocabulary { transform: translateX(0); }
.segment-pill.styles { transform: translateX(100%); }
.segment-pill.features { transform: translateX(200%); }
.tabs button {
  position: relative; z-index: 1; padding: 8px 28px; border: none; background: transparent; height: auto;
  color: #86868b; font-size: 14px; font-weight: 600; border-radius: 10px; box-shadow: none;
}
.tabs button.active { color: #1d1d1f; background: transparent; }

/* 艺术化通用卡片 */
.art-card { background: #fff; border-radius: 20px; box-shadow: 0 4px 24px -6px rgba(0,0,0,0.04); padding: 24px; border: none; min-height: 0; overflow: hidden; }
:deep(.clean-panel.ui-panel) { display: flex; flex-direction: column; min-height: 0; overflow: hidden; padding: 24px; border-radius: 20px; background: #fff; border: none; box-shadow: 0 4px 24px -6px rgba(0,0,0,0.04); }
:deep(.clean-panel .ui-panel-header) { margin-bottom: 18px; display: flex; justify-content: space-between; align-items: center; flex: 0 0 auto; }
:deep(.clean-panel .empty-state) { flex: 1 1 auto; min-height: 0; }
.editor-panel { display: flex; flex-direction: column; min-height: 0; }

/* 头部设计 */
.art-hero { display: flex; align-items: center; gap: 14px; margin-bottom: 20px; width: 100%; flex: 0 0 auto; }
.art-icon-box {
  width: 48px; height: 48px; border-radius: 14px; display: flex; align-items: center; justify-content: center;
  background: linear-gradient(135deg, rgba(15,143,131,0.1), rgba(15,143,131,0.2)); color: #0f8f83;
}
.art-icon-box.editing { background: #f0f0f5; color: #555; }
.art-hero h2 { font-size: 20px; font-weight: 600; color: #1d1d1f; margin: 0; }
.ml-auto { margin-left: auto; }

/* 文本域画布 */
.canvas-wrap { border-radius: 16px; background: #fafafa; padding: 12px; border: 1px solid #f0f0f5; transition: all 0.3s; margin-top: 8px; min-height: 0; }
.canvas-wrap:focus-within { background: #fff; border-color: #0f8f83; box-shadow: 0 0 0 4px rgba(15,143,131,0.1); }
.canvas-textarea { width: 100%; border: none; background: transparent; resize: none; outline: none; font-size: 15px; line-height: 1.6; color: #333; padding: 0; box-shadow: none; }
.canvas-textarea:focus { box-shadow: none; background: transparent; }
.style-editor-panel .form-row { flex: 0 0 auto; }
.style-editor-panel .form-row + .form-row { min-height: 0; }
.style-prompt-textarea { height: 112px; overflow-y: auto; }
.form-row { margin-bottom: 18px; width: 100%; }
.form-footer { margin-top: auto; padding-top: 18px; border-top: 1px solid #f0f0f5; display: flex; justify-content: space-between; align-items: center; width: 100%; flex: 0 0 auto; }
.form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
.field-span-2 { grid-column: 1 / -1; }

/* 词汇气泡 */
.bubble-cloud { display: flex; flex-wrap: wrap; align-content: flex-start; gap: 10px; flex: 1 1 auto; min-height: 0; overflow-y: auto; padding-right: 6px; scrollbar-gutter: stable; }
.bubble-tag {
  display: inline-flex; align-items: center; background: #fff; border: 1px solid rgba(0,0,0,0.08);
  padding: 6px 6px 6px 14px; border-radius: 20px; box-shadow: 0 2px 8px rgba(0,0,0,0.02); transition: transform 0.2s, border-color 0.2s;
}
.bubble-tag:hover { transform: translateY(-2px); border-color: #0f8f83; }
.bubble-tag strong { font-size: 14px; font-weight: 500; color: #333; }
.bubble-close {
  width: 24px; height: 24px; border-radius: 50%; background: #f5f5f5; color: #888; border: none;
  display: flex; align-items: center; justify-content: center; margin-left: 8px;
}
.bubble-close:hover { background: #ffebeb; color: #ff3b30; }

/* 样式列表 */
.style-list { display: flex; flex-direction: column; gap: 12px; flex: 1 1 auto; min-height: 0; overflow-y: auto; padding-right: 6px; scrollbar-gutter: stable; }
.style-item {
  display: flex; justify-content: space-between; align-items: center; padding: 16px 20px;
  background: transparent; border: 1px solid #f0f0f5; border-radius: 16px; text-align: left; transition: all 0.3s; width: 100%;
}
.style-item:hover { background: #fafafa; }
.style-item.active { background: #fff; border-color: #0f8f83; box-shadow: 0 4px 16px rgba(15,143,131,0.08); }
.style-info { display: flex; flex-direction: column; gap: 6px; overflow: hidden; }
.style-info strong { font-size: 15px; font-weight: 600; color: #1d1d1f; }
.style-info em { font-size: 13px; color: #86868b; font-style: normal; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* iOS Switch */
.ios-switch { width: 44px; height: 24px; border-radius: 12px; background: #e5e5ea; position: relative; transition: background 0.3s; flex-shrink: 0; }
.ios-switch::after { content:''; position:absolute; top:2px; left:2px; width:20px; height:20px; background:#fff; border-radius:50%; transition:transform 0.3s cubic-bezier(0.25, 0.8, 0.25, 1); box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
.ios-switch.on { background: #34c759; }
.ios-switch.on::after { transform: translateX(20px); }

.feature-panel { max-width: none; }
.learning-panel { grid-column: 1; grid-row: 1 / span 3; min-height: 0; align-self: start; }
.learning-engine-form { display: grid; gap: 18px; min-height: 0; }
.learning-enable-row { min-height: 74px; }
.learning-form-grid { padding: 0; }
.feature-list { display: grid; gap: 12px; }
.feature-panel, .rules-panel, .corrections-panel { grid-column: 2; min-width: 0; }
.learning-rule-list { display: grid; gap: 10px; max-height: 260px; overflow-y: auto; padding-right: 4px; scrollbar-gutter: stable; }
.learning-rule-item { display: grid; gap: 8px; min-width: 0; padding: 14px; border-radius: 14px; background: #fafafa; border: 1px solid #f0f0f5; }
.learning-rule-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; min-width: 0; }
.learning-rule-head strong { min-width: 0; color: #1d1d1f; font-size: 14px; line-height: 1.45; font-weight: 700; overflow-wrap: anywhere; }
.learning-rule-item p { display: flex; flex-wrap: wrap; gap: 8px 12px; margin: 0; color: #62706c; font-size: 12px; line-height: 1.5; overflow-wrap: anywhere; }
.rule-badge-wrap { flex: 0 0 auto; display: inline-flex; align-items: center; gap: 6px; max-width: min(220px, 46%); position: relative; }
.rule-badge { min-width: 0; border-radius: 999px; padding: 4px 8px; background: rgba(100,116,139,0.1); color: #64748b; font-size: 11px; font-weight: 800; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rule-badge.candidate, .rule-badge.active, .rule-badge.pinned { background: rgba(15,143,131,0.1); color: #08776f; }
.rule-badge.high { background: #f8e6e4; color: #c84d4d; }
.rule-help {
  position: relative; flex: 0 0 auto; width: 18px; height: 18px; display: inline-flex; align-items: center; justify-content: center;
  border-radius: 50%; background: #f0f0f5; color: #86868b; font-size: 11px; cursor: help; outline: none;
}
.rule-help:hover, .rule-help:focus-visible { background: rgba(15,143,131,0.12); color: #08776f; }
.rule-tip {
  position: absolute; z-index: 5; right: 0; top: calc(100% + 8px); width: min(260px, 72vw); padding: 10px 12px;
  border-radius: 10px; background: #1d1d1f; color: #fff; font-size: 12px; line-height: 1.5; font-weight: 500;
  box-shadow: 0 8px 24px rgba(0,0,0,0.16); white-space: normal; overflow-wrap: anywhere;
  opacity: 0; visibility: hidden; transform: translateY(-4px); transition: opacity 0.16s ease, transform 0.16s ease, visibility 0.16s ease;
}
.rule-help:hover .rule-tip, .rule-help:focus-visible .rule-tip { opacity: 1; visibility: visible; transform: translateY(0); }
.correction-record-list { display: grid; gap: 10px; max-height: 360px; overflow-y: auto; padding-right: 4px; scrollbar-gutter: stable; }
.correction-record-item { display: grid; gap: 10px; min-width: 0; padding: 14px; border-radius: 14px; background: #fafafa; border: 1px solid #f0f0f5; }
.correction-record-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.correction-record-head strong { color: #1d1d1f; font-size: 13px; font-weight: 800; }
.correction-diff { display: grid; gap: 8px; }
.correction-diff p { display: grid; grid-template-columns: 42px minmax(0, 1fr); gap: 10px; margin: 0; color: #1d1d1f; font-size: 13px; line-height: 1.55; overflow-wrap: anywhere; }
.correction-diff span { color: #86868b; font-size: 12px; font-weight: 800; }
.correction-record-meta { display: flex; flex-wrap: wrap; gap: 8px 12px; color: #86868b; font-size: 12px; line-height: 1.45; overflow-wrap: anywhere; }
.feature-toggle-row {
  width: 100%; min-height: 78px; display: flex; align-items: center; justify-content: space-between; gap: 20px;
  padding: 18px 20px; background: #fafafa; border: 1px solid #f0f0f5; border-radius: 16px; text-align: left;
}
.feature-toggle-row:hover { background: #fff; border-color: #0f8f83; box-shadow: 0 4px 16px rgba(15,143,131,0.08); }
.feature-toggle-copy { flex: 1 1 auto; min-width: 0; display: flex; flex-direction: column; gap: 6px; white-space: normal; }
.feature-toggle-copy strong { font-size: 15px; font-weight: 600; color: #1d1d1f; white-space: normal; overflow-wrap: anywhere; }
.feature-toggle-copy em { font-size: 13px; line-height: 1.5; color: #86868b; font-style: normal; white-space: normal; overflow-wrap: anywhere; }
.learning-status { max-width: 420px; }

@media (max-width: 900px) {
  .learning-panel,
  .feature-panel,
  .rules-panel,
  .corrections-panel {
    grid-column: 1;
    grid-row: auto;
  }
  .learning-rule-list,
  .correction-record-list {
    max-height: none;
    overflow: visible;
    padding-right: 0;
  }
}

@keyframes fadeUp { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
</style>
