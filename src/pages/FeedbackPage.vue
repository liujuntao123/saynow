<script setup lang="ts">
import AppIcon from '../components/AppIcon.vue';
import EmptyState from '../components/EmptyState.vue';
import PageHeader from '../components/PageHeader.vue';
import UiPanel from '../components/UiPanel.vue';
</script>

<template>
  <div class="page-stack content-centered">
    <PageHeader title="反馈" icon="fileText" />
    <UiPanel class="zen-panel">
      <div class="zen-art-space">
        <div class="art-ring pulse"></div>
        <div class="art-ring static"></div>
        <div class="zen-content">
          <EmptyState icon="fileText" title="功能孕育中" description="当前版本专注打造无缝的语音输入闭环，更多反馈特性敬请期待后续升级。" />
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
      </div>
    </UiPanel>
  </div>
</template>

<style scoped>
.page-stack { display: flex; flex-direction: column; gap: 36px; padding-bottom: 40px; min-height: 80vh; }
.content-centered { flex: 1; display: flex; flex-direction: column; }

:deep(.zen-panel) { background: transparent; border: none; box-shadow: none; display: flex; align-items: center; justify-content: center; flex: 1; padding: 0; }
:deep(.zen-panel .ui-panel-header) { display: none; }

.zen-art-space { position: relative; width: 100%; max-width: 500px; height: 400px; display: flex; align-items: center; justify-content: center; margin: auto; }

.art-ring {
  position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%);
  border-radius: 50%; pointer-events: none;
}
.art-ring.static {
  width: 200px; height: 200px; border: 1px dashed rgba(15,143,131,0.2);
}
.art-ring.pulse {
  width: 280px; height: 280px; background: radial-gradient(circle, rgba(15,143,131,0.03) 0%, transparent 70%);
  animation: breathe 4s ease-in-out infinite;
}

.zen-content { position: relative; z-index: 1; background: rgba(255,255,255,0.8); backdrop-filter: blur(12px); padding: 40px; border-radius: 24px; border: 1px solid rgba(255,255,255,0.5); box-shadow: 0 8px 32px rgba(0,0,0,0.02); text-align: center; }

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

@keyframes breathe {
  0% { transform: translate(-50%, -50%) scale(1); opacity: 0.8; }
  50% { transform: translate(-50%, -50%) scale(1.1); opacity: 0.4; }
  100% { transform: translate(-50%, -50%) scale(1); opacity: 0.8; }
}
</style>
