<script setup lang="ts">
import MetricCard from '../components/MetricCard.vue';
import StatusBadge from '../components/StatusBadge.vue';
import type { DashboardData } from '../types';

defineProps<{
  dashboard: DashboardData | null;
  busy: boolean;
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
    <header class="page-header">
      <div>
        <h1>首页</h1>
        <p>{{ dashboard?.platform.message ?? '正在读取本地状态' }}</p>
      </div>
      <button class="primary-button" type="button" :disabled="busy" @click="emit('simulate')">
        {{ busy ? '识别中' : '模拟一次识别' }}
      </button>
    </header>

    <section class="metrics-grid">
      <MetricCard label="总识别时长" :value="formatDuration(dashboard?.stats.totalDurationSeconds)" hint="成功识别记录累计" />
      <MetricCard label="语音条数" :value="dashboard?.stats.totalRecords ?? 0" hint="失败记录不计入统计" />
      <MetricCard label="转换字数" :value="dashboard?.stats.totalCharacters ?? 0" hint="按 Unicode 字符统计" />
      <MetricCard label="今日状态" value="就绪" hint="按住快捷键开始语音输入" />
    </section>

    <section class="content-section">
      <div class="section-title">
        <h2>最近识别记录</h2>
        <span>{{ dashboard?.records.length ?? 0 }} 条</span>
      </div>
      <div class="records-table">
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
    </section>
  </div>
</template>
