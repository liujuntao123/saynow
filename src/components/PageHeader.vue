<script setup lang="ts">
import AppIcon from './AppIcon.vue';

defineProps<{
  title: string;
  subtitle?: string;
  icon?: string;
}>();
</script>

<template>
  <header class="page-header">
    <div class="page-header-copy">
      <div class="page-title-line">
        <span v-if="icon" class="page-title-icon">
          <AppIcon :name="icon" />
        </span>
        <h1>{{ title }}</h1>
      </div>
      <p v-if="subtitle">{{ subtitle }}</p>
    </div>
    <div v-if="$slots.actions" class="page-header-actions">
      <slot name="actions" />
    </div>
  </header>
</template>
