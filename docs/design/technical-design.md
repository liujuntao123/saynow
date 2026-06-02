# 技术设计

## 技术栈

- 桌面框架：Tauri 2
- 前端：Vue 3 + TypeScript + Vite
- 后端：Rust
- 本地数据库：SQLite
- 测试：Vitest + Cargo test

## 架构

系统分为 UI 层、应用核心层、平台能力层、模型适配层和数据层。

平台能力层封装托盘、全局快捷键、录音和文本注入。Windows 实现接入系统 API；WSL/Linux 实现返回明确的不支持状态，保证开发环境可构建可测试。

模型适配层使用 Provider Adapter 隔离 MiMo 和 Qwen 的调用差异。MVP 以 OpenAI-compatible Chat Completions 为基础，统一处理请求构造、鉴权、响应解析和错误归一化。

## 数据

SQLite 保存供应商配置、快捷键配置、识别记录、词库和风格提示词。API Key 只保存安全存储引用，不在普通表中明文落库。

## 降级

文本写入优先恢复原窗口并模拟粘贴。失败时把识别结果保留到剪贴板，并提示用户手动粘贴。
