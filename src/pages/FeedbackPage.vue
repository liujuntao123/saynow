<script setup lang="ts">
import AppIcon from '../components/AppIcon.vue';
</script>

<template>
  <div class="feedback-page">
    <a
      class="github-link"
      href="https://github.com/liujuntao123/saynow"
      target="_blank"
      rel="noreferrer"
      aria-label="在 GitHub 打开 SayNow 项目"
    >
      <AppIcon name="github" />
      <span>GitHub</span>
    </a>
  </div>
</template>

<style scoped>
.feedback-page { min-height: 80vh; display: flex; align-items: center; justify-content: center; padding-bottom: 40px; }

.github-link {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 40px;
  margin-top: 24px;
  padding: 0 16px;
  border-radius: 12px;
  background: #1d1d1f;
  color: #ffffff;
  font-size: 14px;
  font-weight: 700;
  text-decoration: none;
  box-shadow: 0 8px 20px -8px rgba(29, 29, 31, 0.45);
  transition: transform 0.25s var(--transition-bezier), box-shadow 0.25s var(--transition-bezier), background 0.25s var(--transition-bezier);
}

.github-link:hover {
  background: #2c3331;
  transform: translateY(-2px);
  box-shadow: 0 10px 24px -8px rgba(29, 29, 31, 0.55);
}

.github-link .app-icon {
  font-size: 18px;
}
</style>
