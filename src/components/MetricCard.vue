<script setup lang="ts">
import AppIcon from './AppIcon.vue';

defineProps<{
  label: string;
  value: string | number;
  hint?: string;
  icon?: string;
}>();
</script>

<template>
  <section class="metric-card">
    <span v-if="icon" class="metric-icon">
      <AppIcon :name="icon" />
    </span>
    <span class="metric-label">{{ label }}</span>
    <strong>{{ value }}</strong>
    <small v-if="hint">{{ hint }}</small>
  </section>
</template>
