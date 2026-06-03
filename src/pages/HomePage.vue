<script setup lang="ts">
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import MetricCard from '../components/MetricCard.vue';
import PageHeader from '../components/PageHeader.vue';
import StatusBadge from '../components/StatusBadge.vue';
import UiPanel from '../components/UiPanel.vue';
import type { DashboardData } from '../types';

defineProps<{
  dashboard: DashboardData | null;
  busy: boolean;
  recording: boolean;
}>();

const emit = defineEmits<{
  simulate: [];
}>();

function formatDuration(seconds = 0) {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return minutes > 0 ? `${minutes}分${rest}秒` : `${rest}秒`;
}
</script>

<template>
  <div class="page-stack">
    <PageHeader title="首页" :subtitle="dashboard?.platform.message" icon="mic">
      <template #actions>
        <button class="primary-button icon-button" type="button" :disabled="busy || recording" @click="emit('simulate')">
          <AppIcon :name="busy || recording ? 'activity' : 'mic'" />
          {{ recording ? '录音中' : busy ? '识别中' : '模拟识别' }}
        </button>
      </template>
    </PageHeader>

    <section class="home-summary-grid">
      <UiPanel class="ready-panel">
        <div class="ready-orb" aria-hidden="true">
          <span></span>
        </div>
        <div>
          <span class="eyebrow">今日状态</span>
          <h2>就绪</h2>
        </div>
      </UiPanel>

      <div class="metrics-grid">
        <MetricCard icon="clock" label="总识别时长" :value="formatDuration(dashboard?.stats.totalDurationSeconds)" />
        <MetricCard icon="activity" label="语音条数" :value="dashboard?.stats.totalRecords ?? 0" />
        <MetricCard icon="text" label="转换字数" :value="dashboard?.stats.totalCharacters ?? 0" />
      </div>
    </section>

    <UiPanel title="最近识别记录" :meta="`${dashboard?.records.length ?? 0} 条`" icon="fileText" flush>
      <div v-if="dashboard?.records.length" class="records-table home-records-table">
        <div class="table-row table-head">
          <span>时间</span>
          <span>内容</span>
          <span>模型</span>
          <span>时长</span>
          <span>状态</span>
        </div>
        <div v-for="record in dashboard?.records" :key="record.id" class="table-row">
          <span>{{ new Date(record.createdAt).toLocaleString() }}</span>
          <strong>{{ record.text }}</strong>
          <span>{{ record.model }}</span>
          <span>{{ record.durationSeconds }} 秒</span>
          <StatusBadge :status="record.status" />
        </div>
      </div>
      <EmptyState v-else icon="fileText" title="暂无识别记录" description="完成语音输入后显示。" />
    </UiPanel>
  </div>
</template>
