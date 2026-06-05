<script setup lang="ts">
import { computed } from 'vue';
import AppIcon from './AppIcon.vue';

const props = defineProps<{
  label: string;
  value: string | number;
  hint?: string;
  icon?: string;
}>();

const valueParts = computed(() => {
  const rawValue = String(props.value);
  const parts = rawValue.match(/\d+|[^\d]+/g) ?? [rawValue];
  return parts.map((part) => ({
    text: part,
    isNumber: /^\d+$/.test(part),
  }));
});
</script>

<template>
  <section class="metric-card">
    <span v-if="icon" class="metric-icon">
      <AppIcon :name="icon" />
    </span>
    <span class="metric-label">{{ label }}</span>
    <strong class="metric-value">
      <span
        v-for="(part, index) in valueParts"
        :key="`${part.text}-${index}`"
        :class="part.isNumber ? 'metric-value-number' : 'metric-value-unit'"
      >{{ part.text }}</span>
    </strong>
    <small v-if="hint">{{ hint }}</small>
  </section>
</template>
