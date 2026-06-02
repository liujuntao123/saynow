<script setup lang="ts">
import { reactive, ref } from 'vue';
import StatusBadge from '../components/StatusBadge.vue';
import type { RecognitionRecord, StylePrompt, VocabularyItem } from '../types';

defineProps<{
  records: RecognitionRecord[];
  vocabulary: VocabularyItem[];
  styles: StylePrompt[];
}>();

const emit = defineEmits<{
  addVocabulary: [item: VocabularyItem];
  addStyle: [item: StylePrompt];
}>();

const activeTab = ref<'records' | 'vocabulary' | 'styles'>('records');
const word = reactive<VocabularyItem>({ id: 0, term: '', alias: '', category: '', note: '', enabled: true });
const style = reactive<StylePrompt>({ id: 0, name: '', prompt: '', enabled: true });

function submitVocabulary() {
  if (!word.term.trim()) return;
  emit('addVocabulary', { ...word });
  Object.assign(word, { id: 0, term: '', alias: '', category: '', note: '', enabled: true });
}

function submitStyle() {
  if (!style.name.trim() || !style.prompt.trim()) return;
  emit('addStyle', { ...style });
  Object.assign(style, { id: 0, name: '', prompt: '', enabled: true });
}
</script>

<template>
  <div class="page-stack">
    <header class="page-header">
      <div>
        <h1>数据</h1>
        <p>管理识别记录、用户词库和语音识别风格提示词。</p>
      </div>
    </header>

    <div class="tabs">
      <button :class="{ active: activeTab === 'records' }" type="button" @click="activeTab = 'records'">识别记录</button>
      <button :class="{ active: activeTab === 'vocabulary' }" type="button" @click="activeTab = 'vocabulary'">自定义词库</button>
      <button :class="{ active: activeTab === 'styles' }" type="button" @click="activeTab = 'styles'">风格提示词</button>
    </div>

    <section v-if="activeTab === 'records'" class="content-section">
      <div class="records-table">
        <div class="table-row table-head">
          <span>时间</span>
          <span>文本</span>
          <span>供应商</span>
          <span>状态</span>
        </div>
        <div v-for="record in records" :key="record.id" class="table-row">
          <span>{{ new Date(record.createdAt).toLocaleString() }}</span>
          <strong>{{ record.text }}</strong>
          <span>{{ record.provider }}</span>
          <StatusBadge :status="record.status" />
        </div>
      </div>
    </section>

    <section v-if="activeTab === 'vocabulary'" class="split-section">
      <form class="editor-panel" @submit.prevent="submitVocabulary">
        <label>词条<input v-model="word.term" /></label>
        <label>别名/读音提示<input v-model="word.alias" /></label>
        <label>分类<input v-model="word.category" /></label>
        <label>备注<input v-model="word.note" /></label>
        <label class="check-line"><input v-model="word.enabled" type="checkbox" />启用</label>
        <button class="primary-button" type="submit">添加词条</button>
      </form>
      <div class="list-panel">
        <div v-for="item in vocabulary" :key="item.id" class="data-item">
          <strong>{{ item.term }}</strong>
          <span>{{ item.alias || '无别名' }} · {{ item.category || '未分类' }}</span>
        </div>
      </div>
    </section>

    <section v-if="activeTab === 'styles'" class="split-section">
      <form class="editor-panel" @submit.prevent="submitStyle">
        <label>名称<input v-model="style.name" /></label>
        <label>提示词<textarea v-model="style.prompt" rows="8"></textarea></label>
        <label class="check-line"><input v-model="style.enabled" type="checkbox" />默认启用</label>
        <button class="primary-button" type="submit">添加提示词</button>
      </form>
      <div class="list-panel">
        <div v-for="item in styles" :key="item.id" class="data-item">
          <strong>{{ item.name }}</strong>
          <span>{{ item.prompt }}</span>
        </div>
      </div>
    </section>
  </div>
</template>
