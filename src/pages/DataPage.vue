<script setup lang="ts">
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import StatusBadge from '../components/StatusBadge.vue';
import UiPanel from '../components/UiPanel.vue';
import type { RecognitionRecord } from '../types';

defineProps<{
  records: RecognitionRecord[];
}>();

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}
</script>

<template>
  <div class="page-stack">
    <PageHeader title="数据" icon="database" />

    <UiPanel title="历史识别" :meta="`共 ${records.length} 条记录`" icon="database" class="flat-panel data-record-panel">
      <div v-if="records.length" class="record-card-list art-timeline">
        <article v-for="(record, index) in records" :key="record.id" class="record-card data-record-card" :style="{ animationDelay: `${index * 50}ms` }">
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
              <StatusBadge :status="record.status" />
            </div>

            <p class="transcript-body">{{ record.text }}</p>

            <p v-if="record.errorMessage" class="record-error error-strip">
              <AppIcon name="alert-circle" class="micro-icon" />
              <span>{{ record.errorMessage }}</span>
            </p>
          </div>
        </article>
      </div>
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

.transcript-body { font-size: 15px; line-height: 1.7; color: #333; font-weight: 400; letter-spacing: 0.3px; margin: 0; }

.error-strip {
  margin-top: 16px; display: inline-flex; align-items: center; gap: 8px;
  padding: 10px 16px; background: rgba(255, 59, 48, 0.05); color: #ff3b30 !important;
  border-radius: 10px; font-size: 13px; font-weight: 500;
}

@keyframes fadeSlideUp { from { opacity: 0; transform: translateY(15px); } to { opacity: 1; transform: translateY(0); } }
</style>
