# 说文 saynow

> 一款真正懂你的语音识别输入法

按住快捷键，说完松开，文字自动写入当前输入框。说文是运行在 Windows 桌面端的语音输入助手，常驻托盘，服务于聊天、写代码、记笔记、写文档等高频输入场景。

**把语音输入带到任意输入框。**  
**把专有名词、表达习惯和输出风格变成你的个人配置。**  
**让语音识别越用越像你。**

## 重点亮点

- 全局快捷键录音：在任意应用中按住快捷键开始说话，松开结束。
- 自动写入文本：识别完成后写入当前聚焦输入框，并提供剪贴板备用路径。
- 专属词库：维护术语、缩写、项目名、人名和固定表达，让模型优先识别你常用的词。
- 风格提示词：用自然语言定义输出风格，让同一段语音适配聊天、会议纪要、工单描述或技术记录。
- 自学习引擎：整理明确纠错和成功识别历史，把高频表达、数字格式、技术字段沉淀为个性化规则。
- 模型配置：内置 MiMo-v2.5、MiMo ASR 和 Qwen3.5-Omni 模板，也支持自定义 OpenAI-compatible Base URL、模型和 API Key。
- 历史与统计：查看最近识别记录、使用次数和输入效率数据。

## 开发

安装依赖并启动前端开发服务：

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

需要重点验证的桌面能力包括托盘常驻、全局快捷键、麦克风录音、当前聚焦窗口捕获、文本自动写入，以及剪贴板备用路径。

## 技术实现

前端使用 Vue 3、TypeScript 和 Vite；桌面端基于 Tauri 2；本地能力和数据逻辑主要由 Rust 实现。

后端负责本地命令、SQLite 持久化、模型 Provider 调用、Windows 文本写入，以及个性化上下文构建。SQLite 保存供应商配置、快捷键配置、识别记录、纠错记录、词库、风格提示词、学习引擎配置和学习规则。模型调用以 OpenAI-compatible Chat Completions 为基础，供应商差异主要在 `src-tauri/src/provider.rs` 中适配。

自学习相关逻辑集中在 Rust 后端的数据命令和学习模块中。当前学习闭环包括：保存 `rawText -> correctedText` 纠错对、记录成功识别历史、按空闲时间或手动触发整理、请求配置的学习模型、解析 JSON 学习规则、写入本地规则表，并在后续识别时把可用规则拼入个性化提示上下文。

更多设计细节见：

- `docs/design/prd.md`
- `docs/design/technical-design.md`
- `docs/self-learning-engine.md`
- `docs/development.md`
