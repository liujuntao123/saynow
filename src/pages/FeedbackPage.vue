<script setup lang="ts">
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import UiPanel from '../components/UiPanel.vue';
</script>

<template>
  <div class="page-stack">
    <PageHeader title="反馈" icon="fileText" />
    <UiPanel>
      <EmptyState icon="fileText" title="反馈功能暂未开放" description="当前版本专注语音输入闭环。" />
    </UiPanel>
  </div>
</template>
