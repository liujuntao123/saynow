<script setup lang="ts">
import type { RecognitionStatus } from '../types';

defineProps<{
  status: RecognitionStatus;
}>();
</script>

<template>
  <span class="status-badge" :class="status">
    {{ status === 'success' ? '成功' : status === 'processing' ? '处理中' : '失败' }}
  </span>
</template>
