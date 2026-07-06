<script setup lang="ts">
import { computed, ref } from 'vue';
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import StatusBadge from '../components/StatusBadge.vue';
import UiPanel from '../components/UiPanel.vue';
import { useToast } from '../composables/useToast';
import type { RecognitionRecord } from '../types';

const props = defineProps<{
  records: RecognitionRecord[];
}>();

const emit = defineEmits<{
  deleteRecord: [id: number];
  clearRecords: [];
}>();

const toast = useToast();
const searchText = ref('');
const copiedRecordId = ref<number | null>(null);
let copiedResetTimer: ReturnType<typeof window.setTimeout> | null = null;

const filteredRecords = computed(() => {
  const keyword = searchText.value.trim().toLowerCase();
  if (!keyword) return props.records;
  return props.records.filter(
    (record) =>
      record.text.toLowerCase().includes(keyword) ||
      (record.errorMessage ?? '').toLowerCase().includes(keyword) ||
      record.model.toLowerCase().includes(keyword),
  );
});

// 入场动画延迟封顶，避免长列表末尾条目延迟数秒才出现。
function animationDelay(index: number) {
  return `${Math.min(index, 12) * 40}ms`;
}

async function copyRecordText(record: RecognitionRecord) {
  try {
    await navigator.clipboard.writeText(record.text);
    copiedRecordId.value = record.id;
    if (copiedResetTimer) window.clearTimeout(copiedResetTimer);
    copiedResetTimer = window.setTimeout(() => {
      copiedRecordId.value = null;
    }, 1600);
  } catch (error) {
    toast.error(error, '复制失败');
  }
}

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}
</script>

<template>
  <div class="page-stack">
    <PageHeader title="数据" icon="database" />

    <UiPanel title="历史识别" :meta="`共 ${records.length} 条记录`" icon="database" class="flat-panel data-record-panel">
      <template #headerActions>
        <div class="data-toolbar">
          <div class="search-box">
            <AppIcon name="fileText" class="search-icon" />
            <input v-model="searchText" type="search" placeholder="搜索识别内容…" />
          </div>
          <button
            v-if="records.length"
            class="toolbar-danger-button"
            type="button"
            @click="emit('clearRecords')"
          >
            <AppIcon name="trash" /> 清空
          </button>
        </div>
      </template>

      <div v-if="filteredRecords.length" class="record-card-list art-timeline">
        <article
          v-for="(record, index) in filteredRecords"
          :key="record.id"
          class="record-card data-record-card"
          :style="{ animationDelay: animationDelay(index) }"
        >
          <div class="timeline-line"></div>
          <div class="timeline-dot"></div>

          <div class="data-content glass-card">
            <div class="card-top">
              <div class="info-cluster">
                <span class="info-time">{{ formatDate(record.createdAt) }}</span>
                <span class="info-divider"></span>
                <span class="info-item"><AppIcon name="layers" class="micro-icon"/> {{ record.provider }}</span>
                <span class="info-item">{{ record.model }}</span>
                <span class="info-item">{{ record.durationSeconds }}s</span>
              </div>
              <div class="card-actions">
                <StatusBadge :status="record.status" />
                <button
                  v-if="record.text"
                  type="button"
                  class="record-action-button"
                  :title="copiedRecordId === record.id ? '已复制' : '复制文本'"
                  :aria-label="copiedRecordId === record.id ? '已复制' : '复制文本'"
                  @click="copyRecordText(record)"
                >
                  <AppIcon :name="copiedRecordId === record.id ? 'check' : 'fileText'" />
                </button>
                <button
                  type="button"
                  class="record-action-button danger"
                  title="删除记录"
                  aria-label="删除记录"
                  @click="emit('deleteRecord', record.id)"
                >
                  <AppIcon name="trash" />
                </button>
              </div>
            </div>

            <p class="transcript-body">{{ record.text }}</p>

            <p v-if="record.errorMessage" class="record-error error-strip">
              <AppIcon name="alert-circle" class="micro-icon" />
              <span>{{ record.errorMessage }}</span>
            </p>
          </div>
        </article>
      </div>
      <EmptyState
        v-else-if="records.length"
        icon="fileText"
        title="没有匹配的记录"
        description="换个关键词试试。"
      />
      <EmptyState v-else icon="fileText" title="暂无识别记录" description="完成语音识别后将在此处凝结成文字。" />
    </UiPanel>
  </div>
</template>

<style scoped>
.page-stack { display: flex; flex-direction: column; gap: 24px; min-height: 0; height: 100%; overflow: hidden; }

/* 扁平无界容器 */
:deep(.flat-panel) { background: transparent; box-shadow: none; border: none; padding: 0; }
:deep(.flat-panel .ui-panel-header) { font-size: 20px; padding-bottom: 14px; border-bottom: 2px solid rgba(0,0,0,0.03); margin-bottom: 18px; display: flex; justify-content: space-between; align-items: center; flex: 0 0 auto; }
:deep(.data-record-panel) { display: flex; flex-direction: column; flex: 1 1 auto; min-height: 0; overflow: hidden; }
:deep(.data-record-panel .empty-state) { flex: 1 1 auto; min-height: 0; }

/* 工具栏 */
.data-toolbar { display: flex; align-items: center; gap: 10px; }
.search-box {
  display: flex; align-items: center; gap: 8px; min-width: 220px;
  background: #ffffff; border: 1px solid rgba(0,0,0,0.08); border-radius: 10px; padding: 0 10px;
}
.search-box:focus-within { border-color: #0f8f83; box-shadow: 0 0 0 3px rgba(15,143,131,0.1); }
.search-icon { width: 14px; height: 14px; color: #86868b; flex: 0 0 auto; }
.search-box input {
  border: none; background: transparent; box-shadow: none; padding: 8px 0; font-size: 13px; width: 100%;
}
.search-box input:focus { border: none; box-shadow: none; background: transparent; }
.toolbar-danger-button {
  display: inline-flex; align-items: center; gap: 6px; border: none; cursor: pointer;
  background: #f8e6e4; color: #c84d4d; border-radius: 10px; padding: 8px 14px; font-size: 13px; font-weight: 600;
  transition: background 0.2s ease;
}
.toolbar-danger-button:hover { background: #f0caca; }

/* 优雅时间轴复用原本类名进行覆盖 */
.art-timeline { position: relative; padding-left: 20px; padding-right: 8px; display: flex; flex-direction: column; gap: 18px; flex: 1 1 auto; min-height: 0; overflow-y: auto; scrollbar-gutter: stable; }
.data-record-card { position: relative; padding-left: 36px; animation: fadeSlideUp 0.6s ease backwards; border: none; background: transparent; box-shadow: none; display: block; }
.data-record-card:hover { background: transparent; transform: none; box-shadow: none; }

/* 时间轴线条和圆点 */
.timeline-line {
  position: absolute; left: 0; top: 32px; bottom: -56px; width: 2px;
  background: linear-gradient(to bottom, rgba(15,143,131,0.2) 0%, rgba(15,143,131,0.05) 100%);
  border-radius: 2px;
}
.data-record-card:last-child .timeline-line { display: none; }
.timeline-dot {
  position: absolute; left: -4px; top: 20px; width: 10px; height: 10px; border-radius: 50%;
  background: #fff; border: 2px solid var(--accent, #0f8f83);
  box-shadow: 0 0 0 4px rgba(15,143,131,0.1);
}

/* 玻璃态内容卡片 */
.glass-card {
  background: #ffffff; border-radius: 16px; padding: 20px;
  border: 1px solid rgba(0,0,0,0.03); box-shadow: 0 4px 24px -8px rgba(0,0,0,0.04);
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}
.glass-card:hover { transform: translateX(4px); box-shadow: 0 12px 32px -8px rgba(0,0,0,0.08); border-color: rgba(15,143,131,0.1); }

.card-top { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 12px; flex-wrap: wrap; gap: 12px; }
.info-cluster { display: flex; align-items: center; flex-wrap: wrap; gap: 10px; }
.info-time { font-size: 14px; font-weight: 600; color: #1d1d1f; }
.info-divider { width: 4px; height: 4px; border-radius: 50%; background: #d1d1d6; }
.info-item { display: inline-flex; align-items: center; gap: 4px; font-size: 13px; color: #86868b; }
.micro-icon { width: 14px; height: 14px; }

.card-actions { display: inline-flex; align-items: center; gap: 8px; }
.record-action-button {
  display: grid; place-items: center; width: 28px; height: 28px; border: none; border-radius: 8px;
  background: #f5f7f6; color: #62706c; cursor: pointer; padding: 0; transition: all 0.2s ease;
}
.record-action-button:hover { background: rgba(15,143,131,0.1); color: #0f8f83; }
.record-action-button.danger:hover { background: #f8e6e4; color: #c84d4d; }
.record-action-button .app-icon { width: 14px; height: 14px; }

.transcript-body { font-size: 15px; line-height: 1.7; color: #333; font-weight: 400; letter-spacing: 0.3px; margin: 0; }

.error-strip {
  margin-top: 16px; display: inline-flex; align-items: center; gap: 8px;
  padding: 10px 16px; background: rgba(255, 59, 48, 0.05); color: #ff3b30 !important;
  border-radius: 10px; font-size: 13px; font-weight: 500;
}

@keyframes fadeSlideUp { from { opacity: 0; transform: translateY(15px); } to { opacity: 1; transform: translateY(0); } }
</style>
