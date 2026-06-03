<script setup lang="ts">
import AppIcon from './AppIcon.vue';

defineProps<{
  title?: string;
  meta?: string;
  flush?: boolean;
  icon?: string;
}>();
</script>

<template>
  <section class="ui-panel" :class="{ 'ui-panel-flush': flush }">
    <div v-if="title || meta || $slots.headerActions" class="ui-panel-header">
      <div class="ui-panel-heading">
        <span v-if="icon" class="panel-icon">
          <AppIcon :name="icon" />
        </span>
        <h2 v-if="title">{{ title }}</h2>
        <span v-if="meta">{{ meta }}</span>
      </div>
      <div v-if="$slots.headerActions" class="ui-panel-actions">
        <slot name="headerActions" />
      </div>
    </div>
    <slot />
  </section>
</template>
