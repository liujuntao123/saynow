<script setup lang="ts">
import EmptyState from '../components/EmptyState.vue';
import MetricCard from '../components/MetricCard.vue';
import PageHeader from '../components/PageHeader.vue';
import StatusBadge from '../components/StatusBadge.vue';
import UiPanel from '../components/UiPanel.vue';
import type { DashboardData } from '../types';

defineProps<{
  dashboard: DashboardData | null;
}>();

function formatDuration(seconds = 0) {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return minutes > 0 ? `${minutes}分${rest}秒` : `${rest}秒`;
}

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}
</script>

<template>
  <div class="page-stack">
    <PageHeader title="首页" :subtitle="dashboard?.platform.message" icon="home" />

    <section class="metrics-grid metrics-grid-home">
      <MetricCard icon="clock" label="总识别时长" :value="formatDuration(dashboard?.stats.totalDurationSeconds)" />
      <MetricCard icon="activity" label="语音条数" :value="dashboard?.stats.totalRecords ?? 0" />
      <MetricCard icon="text" label="转换字数" :value="dashboard?.stats.totalCharacters ?? 0" />
    </section>

    <UiPanel title="最近识别记录" :meta="`${dashboard?.records.length ?? 0} 条`" icon="fileText" class="glass-panel home-record-panel">
      <div v-if="dashboard?.records.length" class="record-card-list compact-record-list">
        <article v-for="(record, index) in dashboard.records" :key="record.id" class="record-card" :style="{ animationDelay: `${index * 40}ms` }">
          <div class="record-card-main">
            <p>{{ record.text }}</p>
            <div class="record-card-tags">
              <span class="meta-chip"><i class="dot-time"></i>{{ formatDate(record.createdAt) }}</span>
              <span class="meta-chip">{{ record.model }}</span>
              <span class="meta-chip">{{ record.durationSeconds }} 秒</span>
            </div>
          </div>
          <div class="status-wrap">
            <StatusBadge :status="record.status" />
          </div>
        </article>
      </div>
      <EmptyState v-else icon="fileText" title="暂无识别记录" description="完成语音输入后显示。" />
    </UiPanel>
  </div>
</template>

<style scoped>
/* 页面基础排版 */
.page-stack { display: flex; flex-direction: column; gap: 24px; min-width: 0; min-height: 0; height: 100%; overflow: hidden; }

/* 数据卡片网格 */
.metrics-grid { display: grid; gap: 24px; grid-template-columns: repeat(3, minmax(0, 1fr)); flex: 0 0 auto; }
:deep(.metric-card) {
  background: #ffffff;
  backdrop-filter: none;
  border: 1px solid rgba(255,255,255,0.78);
  box-shadow: 0 18px 38px -24px rgba(11, 49, 42, 0.32);
  border-radius: 18px;
  transition: transform 0.3s cubic-bezier(0.25, 0.8, 0.25, 1), box-shadow 0.3s ease;
  padding: 28px 32px 26px;
  min-height: 190px;
  display: grid;
  grid-template-rows: 44px 30px 1fr;
  align-items: start;
  gap: 16px;
  overflow: hidden;
}
:deep(.metric-card):hover { transform: translateY(-3px); box-shadow: 0 24px 46px -24px rgba(11, 49, 42, 0.38); }
:deep(.metric-card::after) { display: none; }
:deep(.metric-icon) {
  width: 44px;
  height: 44px;
  margin: 0;
  color: #195f55;
  font-size: 22px;
  background: #eef5f3;
  border-radius: 11px;
  box-shadow: none;
}
:deep(.metric-label) {
  color: #49645d;
  font-size: 16px;
  font-weight: 500;
  line-height: 1.4;
}
:deep(.metric-value) {
  align-self: end;
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 7px;
  color: #1f5d4d;
  font-weight: 900;
  line-height: 0.94;
  letter-spacing: 0;
  background: none;
  -webkit-text-fill-color: currentColor;
}
:deep(.metric-value-number) {
  font-size: 58px;
  letter-spacing: -1px;
}
:deep(.metric-value-unit) {
  color: #153a32;
  font-size: 20px;
  font-weight: 800;
  letter-spacing: 0;
}
@media (max-width: 1080px) {
  .metrics-grid { grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); }
  :deep(.metric-card) { min-height: 172px; padding: 24px; }
  :deep(.metric-value-number) { font-size: 48px; }
}

/* 面板深度定制 */
:deep(.glass-panel) { background: transparent; border: none; box-shadow: none; padding: 0; }
:deep(.glass-panel .ui-panel-header) { margin-bottom: 16px; padding-bottom: 12px; border-bottom: 1px solid rgba(0,0,0,0.04); display: flex; align-items: center; justify-content: space-between; flex: 0 0 auto; }
:deep(.home-record-panel) { display: flex; flex-direction: column; flex: 1 1 auto; min-height: 0; overflow: hidden; }
:deep(.home-record-panel .empty-state) { flex: 1 1 auto; min-height: 0; }

/* 列表卡片艺术化 */
.record-card-list { display: flex; flex-direction: column; gap: 16px; }
.compact-record-list { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding-right: 6px; scrollbar-gutter: stable; }
.record-card {
  display: flex; justify-content: space-between; align-items: flex-start; gap: 20px;
  padding: 18px 20px; background: rgba(255, 255, 255, 0.6); backdrop-filter: blur(12px);
  border-radius: 16px; border: 1px solid rgba(255,255,255,0.8);
  box-shadow: 0 4px 20px -2px rgba(0,0,0,0.02);
  animation: slideUpFade 0.6s cubic-bezier(0.16, 1, 0.3, 1) backwards;
  transition: all 0.3s ease;
}
.record-card:hover { background: #ffffff; transform: translateY(-2px); box-shadow: 0 12px 30px -4px rgba(15,143,131,0.06); }
.record-card-main { flex: 1; display: flex; flex-direction: column; gap: 10px; min-width: 0; }
.record-card p { margin: 0; font-size: 15px; line-height: 1.6; color: #1d1d1f; letter-spacing: 0.2px; font-weight: 400; }

/* 优雅的标签 */
.record-card-tags { display: flex; flex-wrap: wrap; gap: 12px; align-items: center; }
.meta-chip {
  display: inline-flex; align-items: center; font-size: 12px; font-weight: 500;
  color: #62706c; background: rgba(15,143,131,0.04); padding: 5px 12px; border-radius: 8px; border: none;
}
.dot-time { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: #0f8f83; margin-right: 6px; box-shadow: 0 0 8px rgba(15,143,131,0.4); }
.status-wrap { flex-shrink: 0; }

@keyframes slideUpFade {
  from { opacity: 0; transform: translateY(20px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
