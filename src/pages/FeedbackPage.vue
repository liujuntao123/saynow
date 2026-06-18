<script setup lang="ts">
import AppIcon from '../components/AppIcon.vue';
import PageHeader from '../components/PageHeader.vue';
import { openExternalUrl } from '../api/tauri';
import packageJson from '../../package.json';

const appVersion = packageJson.version;
const githubUrl = 'https://github.com/liujuntao123/saynow';
const downloadUrl = `${githubUrl}/releases`;
const authorQrPath = '/author-contact-qr.png';

function openLink(url: string) {
  void openExternalUrl(url);
}
</script>

<template>
  <div class="page-stack about-page">
    <PageHeader title="关于" icon="question" />

    <section class="about-grid">
      <article class="about-card app-card">
        <div class="app-mark">语</div>
        <div class="app-copy">
          <span>saynow</span>
          <strong>说文</strong>
          <em>当前版本 v{{ appVersion }}</em>
        </div>
      </article>

      <article class="about-card link-card">
        <div class="section-heading">
          <span class="section-icon"><AppIcon name="github" /></span>
          <h2>项目链接</h2>
        </div>
        <div class="link-list">
          <button type="button" class="about-link primary" @click="openLink(downloadUrl)">
            <AppIcon name="download" />
            <span>下载最新版本</span>
          </button>
          <button type="button" class="about-link" @click="openLink(githubUrl)">
            <AppIcon name="github" />
            <span>GitHub 仓库</span>
          </button>
        </div>
      </article>

      <article class="about-card contact-card">
        <div class="section-heading">
          <span class="section-icon"><AppIcon name="message" /></span>
          <h2>作者联系方式</h2>
        </div>
        <div
          class="qr-slot"
          :style="{ backgroundImage: `url(${authorQrPath})` }"
          role="img"
          aria-label="作者联系方式二维码"
        ></div>
      </article>
    </section>
  </div>
</template>

<style scoped>
.about-page {
  gap: 24px;
}

.about-grid {
  display: grid;
  grid-template-columns: minmax(260px, 0.9fr) minmax(320px, 1.1fr);
  gap: 18px;
  min-height: 0;
}

.about-card {
  border: 1px solid rgba(255, 255, 255, 0.82);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 0 18px 38px -24px rgba(11, 49, 42, 0.32);
  padding: 24px;
}

.app-card {
  display: flex;
  align-items: center;
  gap: 18px;
  min-height: 170px;
  background: #ffffff;
}

.app-mark {
  display: grid;
  width: 68px;
  height: 68px;
  place-items: center;
  border-radius: 18px;
  color: #ffffff;
  font-size: 34px;
  font-weight: 850;
  background: linear-gradient(135deg, #2da79b, #08756f);
  box-shadow: 0 16px 30px -12px rgba(8, 117, 111, 0.46);
}

.app-copy {
  display: grid;
  gap: 7px;
}

.app-copy span,
.app-copy em {
  color: #62706c;
  font-size: 13px;
  font-style: normal;
  font-weight: 650;
}

.app-copy strong {
  color: #1d1d1f;
  font-size: 30px;
  line-height: 1;
  font-weight: 850;
  letter-spacing: 0;
}

.link-card,
.contact-card {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.contact-card {
  grid-column: 1 / -1;
}

.section-heading {
  display: flex;
  align-items: center;
  gap: 10px;
}

.section-heading h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 750;
}

.section-icon {
  display: grid;
  width: 34px;
  height: 34px;
  place-items: center;
  border-radius: 10px;
  color: #08776f;
  background: rgba(15, 143, 131, 0.1);
}

.link-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.about-link {
  border: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 44px;
  border-radius: 12px;
  padding: 0 16px;
  color: #08776f;
  background: rgba(15, 143, 131, 0.1);
  font-size: 14px;
  font-weight: 750;
  transition: transform 0.25s var(--transition-bezier), box-shadow 0.25s var(--transition-bezier), background 0.25s var(--transition-bezier);
}

.about-link.primary {
  color: #ffffff;
  background: #1d1d1f;
  box-shadow: 0 10px 22px -12px rgba(29, 29, 31, 0.52);
}

.about-link:hover {
  transform: translateY(-2px);
  background: #d8efeb;
}

.about-link.primary:hover {
  background: #2c3331;
}

.qr-slot {
  display: grid;
  width: 180px;
  height: 180px;
  place-items: center;
  align-self: start;
  border: 1px dashed rgba(15, 143, 131, 0.32);
  border-radius: 16px;
  background-color: #ffffff;
  background-position: center;
  background-repeat: no-repeat;
  background-size: cover;
  color: #62706c;
  text-align: center;
}

@media (max-width: 1080px) {
  .about-grid,
  .link-list {
    grid-template-columns: 1fr;
  }

  .contact-card {
    grid-column: auto;
  }
}
</style>
