<script setup lang="ts">
import AppIcon from './AppIcon.vue';

defineProps<{
  title: string;
  description: string;
  icon?: string;
}>();
</script>

<template>
  <div class="empty-state">
    <span v-if="icon" class="empty-icon">
      <AppIcon :name="icon" />
    </span>
    <strong>{{ title }}</strong>
    <span>{{ description }}</span>
  </div>
</template>
