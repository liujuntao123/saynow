<script setup lang="ts">
import { reactive, ref } from 'vue';
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import StatusBadge from '../components/StatusBadge.vue';
import UiPanel from '../components/UiPanel.vue';
import type { RecognitionRecord, StylePrompt, VocabularyItem } from '../types';

defineProps<{
  records: RecognitionRecord[];
  vocabulary: VocabularyItem[];
  styles: StylePrompt[];
}>();

const emit = defineEmits<{
  addVocabularyTerms: [terms: string[]];
  deleteVocabulary: [id: number];
  addStyle: [item: StylePrompt];
  updateStyle: [item: StylePrompt];
  deleteStyle: [id: number];
}>();

const activeTab = ref<'records' | 'vocabulary' | 'styles'>('records');
const vocabularyText = ref('');
const style = reactive<StylePrompt>({ id: 0, name: '', prompt: '', enabled: true });
const editingStyles = reactive<Record<number, StylePrompt>>({});

function submitVocabulary() {
  const terms = vocabularyText.value
    .split('\n')
    .map((term) => term.trim())
    .filter(Boolean);
  if (!terms.length) return;
  emit('addVocabularyTerms', terms);
  vocabularyText.value = '';
}

function submitStyle() {
  if (!style.name.trim() || !style.prompt.trim()) return;
  emit('addStyle', { ...style });
  Object.assign(style, { id: 0, name: '', prompt: '', enabled: true });
}

function startEditStyle(item: StylePrompt) {
  editingStyles[item.id] = { ...item };
}

function cancelEditStyle(id: number) {
  delete editingStyles[id];
}

function saveStyle(id: number) {
  const draft = editingStyles[id];
  if (!draft?.name.trim() || !draft.prompt.trim()) return;
  emit('updateStyle', { ...draft, name: draft.name.trim(), prompt: draft.prompt.trim() });
  delete editingStyles[id];
}

function toggleStyle(item: StylePrompt) {
  emit('updateStyle', { ...item, enabled: !item.enabled });
}
</script>

<template>
  <div class="page-stack">
    <PageHeader title="数据" icon="database" />

    <div class="tabs">
      <button :class="{ active: activeTab === 'records' }" type="button" @click="activeTab = 'records'">
        <AppIcon name="fileText" />
        识别记录
      </button>
      <button :class="{ active: activeTab === 'vocabulary' }" type="button" @click="activeTab = 'vocabulary'">
        <AppIcon name="book" />
        自定义词库
      </button>
      <button :class="{ active: activeTab === 'styles' }" type="button" @click="activeTab = 'styles'">
        <AppIcon name="spark" />
        风格提示词
      </button>
    </div>

    <UiPanel v-if="activeTab === 'records'" :meta="`共 ${records.length} 条记录`" flush>
      <div v-if="records.length" class="records-table data-records-table">
        <div class="table-row table-head data-table-row">
          <span>时间</span>
          <span>文本</span>
          <span>供应商</span>
          <span>状态</span>
        </div>
        <div v-for="record in records" :key="record.id" class="table-row data-table-row">
          <span>{{ new Date(record.createdAt).toLocaleString() }}</span>
          <strong>{{ record.text }}</strong>
          <span>{{ record.provider }}</span>
          <StatusBadge :status="record.status" />
        </div>
      </div>
      <EmptyState v-else icon="fileText" title="暂无识别记录" description="完成语音识别后显示。" />
    </UiPanel>

    <section v-if="activeTab === 'vocabulary'" class="split-section">
      <form class="editor-panel" @submit.prevent="submitVocabulary">
        <div class="panel-title-block">
          <span class="panel-title-icon"><AppIcon name="book" /></span>
          <h2>自定义词条</h2>
        </div>
        <label>
          词条内容
          <textarea v-model="vocabularyText" rows="9" placeholder="每行一个词条"></textarea>
        </label>
        <button class="primary-button icon-button full-width" type="submit">
          <AppIcon name="plus" />
          添加词条
        </button>
      </form>
      <UiPanel class="list-panel" title="词库条目" :meta="`共 ${vocabulary.length} 条`" icon="database">
        <div v-if="vocabulary.length" class="data-list">
          <div v-for="item in vocabulary" :key="item.id" class="data-item">
            <div>
              <strong>{{ item.term }}</strong>
              <span class="state-pill" :class="{ disabled: !item.enabled }">{{ item.enabled ? '已启用' : '已停用' }}</span>
            </div>
            <button class="danger-button icon-only-button" type="button" aria-label="删除词条" title="删除" @click="emit('deleteVocabulary', item.id)">
              <AppIcon name="trash" />
            </button>
          </div>
        </div>
        <EmptyState v-else icon="book" title="暂无词条" description="添加后用于识别保留。" />
      </UiPanel>
    </section>

    <section v-if="activeTab === 'styles'" class="split-section">
      <form class="editor-panel" @submit.prevent="submitStyle">
        <div class="panel-title-block">
          <span class="panel-title-icon"><AppIcon name="spark" /></span>
          <h2>新建提示词</h2>
        </div>
        <label>
          名称
          <input v-model="style.name" />
        </label>
        <label>
          提示词
          <textarea v-model="style.prompt" rows="8"></textarea>
        </label>
        <label class="check-line"><input v-model="style.enabled" type="checkbox" />默认启用</label>
        <button class="primary-button icon-button full-width" type="submit">
          <AppIcon name="plus" />
          添加提示词
        </button>
      </form>
      <UiPanel class="list-panel" title="提示词预设" :meta="`共 ${styles.length} 条`" icon="spark">
        <div v-if="styles.length" class="data-list">
          <div v-for="item in styles" :key="item.id" class="data-item style-item">
            <template v-if="editingStyles[item.id]">
              <label>名称<input v-model="editingStyles[item.id].name" /></label>
              <label>提示词<textarea v-model="editingStyles[item.id].prompt" rows="5"></textarea></label>
              <label class="check-line"><input v-model="editingStyles[item.id].enabled" type="checkbox" />启用</label>
              <div class="item-actions">
                <button class="primary-button icon-button" type="button" @click="saveStyle(item.id)">
                  <AppIcon name="save" />
                  保存
                </button>
                <button class="ghost-button icon-only-button" type="button" aria-label="取消编辑" title="取消" @click="cancelEditStyle(item.id)">
                  <AppIcon name="x" />
                </button>
              </div>
            </template>
            <template v-else>
              <div class="item-heading">
                <div>
                  <strong>{{ item.name }}</strong>
                  <p>{{ item.prompt }}</p>
                </div>
                <span class="state-pill" :class="{ disabled: !item.enabled }">{{ item.enabled ? '已启用' : '已停用' }}</span>
              </div>
              <div class="item-actions">
                <button class="secondary-button icon-button" type="button" @click="startEditStyle(item)">
                  <AppIcon name="settings" />
                  修改
                </button>
                <button class="ghost-button icon-button" type="button" @click="toggleStyle(item)">
                  <AppIcon :name="item.enabled ? 'x' : 'check'" />
                  {{ item.enabled ? '停用' : '启用' }}
                </button>
                <button class="danger-button icon-only-button" type="button" aria-label="删除提示词" title="删除" @click="emit('deleteStyle', item.id)">
                  <AppIcon name="trash" />
                </button>
              </div>
            </template>
          </div>
        </div>
        <EmptyState v-else icon="spark" title="暂无提示词" description="添加后用于文本整理。" />
      </UiPanel>
    </section>
  </div>
</template>
