<script setup lang="ts">
import {
  PhBookOpen,
  PhArrowCounterClockwise,
  PhCheck,
  PhClock,
  PhDatabase,
  PhFileText,
  PhFloppyDisk,
  PhGithubLogo,
  PhHouse,
  PhKeyboard,
  PhKey,
  PhMicrophone,
  PhChatText,
  PhPlus,
  PhPulse,
  PhSparkle,
  PhStack,
  PhTextT,
  PhTrash,
  PhWarningCircle,
  PhX,
  PhGearSix,
} from '@phosphor-icons/vue';
import { computed } from 'vue';

const props = defineProps<{
  name: string;
}>();

const icons = {
  activity: PhPulse,
  undo: PhArrowCounterClockwise,
  book: PhBookOpen,
  check: PhCheck,
  clock: PhClock,
  database: PhDatabase,
  fileText: PhFileText,
  github: PhGithubLogo,
  home: PhHouse,
  keyboard: PhKeyboard,
  key: PhKey,
  layers: PhStack,
  mic: PhMicrophone,
  message: PhChatText,
  plus: PhPlus,
  save: PhFloppyDisk,
  settings: PhGearSix,
  spark: PhSparkle,
  text: PhTextT,
  trash: PhTrash,
  'alert-circle': PhWarningCircle,
  x: PhX,
} as const;

const icon = computed(() => icons[props.name as keyof typeof icons] ?? PhSparkle);
</script>

<template>
  <component :is="icon" class="app-icon" :class="`app-icon-${name}`" :size="'1em'" :weight="'bold'" aria-hidden="true" />
</template>
