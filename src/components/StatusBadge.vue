<script setup lang="ts">
import AppIcon from './AppIcon.vue';
import type { RecognitionStatus } from '../types';

defineProps<{
  status: RecognitionStatus;
}>();
</script>

<template>
  <span class="status-badge" :class="status">
    <AppIcon :name="status === 'failed' ? 'x' : status === 'processing' ? 'activity' : 'check'" />
    {{ status === 'success' ? '成功' : status === 'processing' ? '处理中' : '失败' }}
  </span>
</template>
