# 说文 saynow

## 产品用户

### 一句话介绍

说文是一个 Windows 桌面端语音输入助手：按住快捷键说话，松开后自动识别语音，并把文字写入当前输入框。

### 产品特性

- 全局快捷键录音：无需切换应用，在任意输入场景中直接触发语音输入。
- 自动写入文本：识别完成后写入当前聚焦输入框，写入失败时可降级复制到剪贴板。
- 多模型配置：内置 MiMo-v2.5、MiMo ASR 和 Qwen3.5-Omni 配置模板，支持自定义 Base URL、模型和 API Key。
- 个性化识别：支持自定义词库和风格提示词，让识别结果更贴近日常表达。
- 历史与统计：查看最近识别记录、使用次数和输入效率数据。

### 产品亮点

- 面向真实桌面输入流程，不只是一个语音转文字页面。
- 把快捷键、录音、识别、文本注入串成一个闭环，减少复制粘贴。
- 配置开放，方便接入 OpenAI-compatible 的多模态语音模型。

## 产品开发者

### 快速开始开发

```bash
npm install
npm run dev
```

常用验证命令：

```bash
npm test -- --run
npm run build
cd src-tauri && cargo test
```

Windows 桌面能力需要在 Windows 原生环境验证：

```bash
npm run tauri dev -- --features desktop
```

### 基本技术介绍

- 前端：Vue 3、TypeScript、Vite、Vitest。
- 桌面端：Tauri 2，负责窗口、托盘、全局快捷键和前后端命令通信。
- 后端：Rust，负责本地命令、SQLite 持久化、模型 Provider 调用和 Windows 文本写入能力。
- 数据：SQLite 保存配置、识别记录、词库、风格提示词和使用统计。
- 模型接入：以 OpenAI-compatible Chat Completions 为基础，供应商差异在 `src-tauri/src/provider.rs` 中适配。
