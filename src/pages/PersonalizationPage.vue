<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import UiPanel from '../components/UiPanel.vue';
import type { StylePrompt, VocabularyItem } from '../types';

const props = defineProps<{ vocabulary: VocabularyItem[]; styles: StylePrompt[]; }>();
const emit = defineEmits<{ addVocabularyTerms: [terms: string[]]; deleteVocabulary: [id: number]; addStyle: [item: StylePrompt]; updateStyle: [item: StylePrompt]; deleteStyle: [id: number]; }>();

const activeTab = ref<'vocabulary' | 'styles'>('vocabulary');
const vocabularyText = ref('');
const styleForm = reactive<StylePrompt>({ id: 0, name: '', prompt: '', enabled: true });
const selectedStyleId = ref<number | null>(null);
watch(() => props.styles, (items) => {
  const current = items.find((item) => item.id === selectedStyleId.value);
  const next = current ?? items.find((item) => item.enabled) ?? items[0];
  selectedStyleId.value = next?.id ?? null; Object.assign(styleForm, next ?? { id: 0, name: '', prompt: '', enabled: true });
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
              <textarea v-model="styleForm.prompt" rows="8" class="canvas-textarea" placeholder="描述你希望 AI 如何润色和整理文本..."></textarea>
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
.page-stack { display: flex; flex-direction: column; gap: 36px; padding-bottom: 40px; align-items: stretch; }
.art-board { width: 100%; display: grid; grid-template-columns: 1fr 1fr; gap: 24px; align-items: start; animation: fadeUp 0.4s ease; }
.split-section { grid-template-columns: minmax(280px, 0.72fr) minmax(0, 1.28fr); }
.style-management { grid-template-columns: minmax(260px, 0.66fr) minmax(0, 1.34fr); }
@media (max-width: 900px) { .art-board { grid-template-columns: 1fr; } .reverse-on-mobile { display: flex; flex-direction: column-reverse; } }

/* 分段控制器覆盖原本 tabs */
.tabs.segmented-control {
  position: relative; display: inline-flex; align-self: flex-start; background: #f0f0f5; padding: 4px;
  border-radius: 14px; border: none; box-shadow: inset 0 2px 4px rgba(0,0,0,0.04); margin: -10px 0 8px;
}
.segment-pill {
  position: absolute; top: 4px; bottom: 4px; width: calc(50% - 4px); background: #ffffff;
  border-radius: 10px; box-shadow: 0 2px 8px rgba(0,0,0,0.08); transition: transform 0.4s cubic-bezier(0.25, 0.8, 0.25, 1);
}
.segment-pill.vocabulary { transform: translateX(0); }
.segment-pill.styles { transform: translateX(100%); }
.tabs button {
  position: relative; z-index: 1; padding: 8px 32px; border: none; background: transparent; height: auto;
  color: #86868b; font-size: 14px; font-weight: 600; border-radius: 10px; box-shadow: none;
}
.tabs button.active { color: #1d1d1f; background: transparent; }

/* 艺术化通用卡片 */
.art-card { background: #fff; border-radius: 20px; box-shadow: 0 4px 24px -6px rgba(0,0,0,0.04); padding: 32px; border: none; }
:deep(.clean-panel.ui-panel) { padding: 32px; border-radius: 20px; background: #fff; border: none; box-shadow: 0 4px 24px -6px rgba(0,0,0,0.04); }
:deep(.clean-panel .ui-panel-header) { margin-bottom: 24px; display: flex; justify-content: space-between; align-items: center; }

/* 头部设计 */
.art-hero { display: flex; align-items: center; gap: 16px; margin-bottom: 32px; width: 100%; }
.art-icon-box {
  width: 48px; height: 48px; border-radius: 14px; display: flex; align-items: center; justify-content: center;
  background: linear-gradient(135deg, rgba(15,143,131,0.1), rgba(15,143,131,0.2)); color: #0f8f83;
}
.art-icon-box.editing { background: #f0f0f5; color: #555; }
.art-hero h2 { font-size: 20px; font-weight: 600; color: #1d1d1f; margin: 0; }
.ml-auto { margin-left: auto; }

/* 文本域画布 */
.canvas-wrap { border-radius: 16px; background: #fafafa; padding: 16px; border: 1px solid #f0f0f5; transition: all 0.3s; margin-top: 8px; }
.canvas-wrap:focus-within { background: #fff; border-color: #0f8f83; box-shadow: 0 0 0 4px rgba(15,143,131,0.1); }
.canvas-textarea { width: 100%; border: none; background: transparent; resize: none; outline: none; font-size: 15px; line-height: 1.6; color: #333; padding: 0; box-shadow: none; }
.canvas-textarea:focus { box-shadow: none; background: transparent; }
.form-row { margin-bottom: 24px; width: 100%; }
.form-footer { margin-top: 32px; padding-top: 24px; border-top: 1px solid #f0f0f5; display: flex; justify-content: space-between; align-items: center; width: 100%; }

/* 词汇气泡 */
.bubble-cloud { display: flex; flex-wrap: wrap; gap: 10px; }
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
.style-list { display: flex; flex-direction: column; gap: 12px; }
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

@keyframes fadeUp { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: translateY(0); } }
</style>
