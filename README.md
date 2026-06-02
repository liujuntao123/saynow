# 语音输入助手

Windows 桌面端语音识别输入工具。MVP 使用 Tauri 2、Vue 3、Rust 和 SQLite 构建，核心流程是全局快捷键录音、调用在线多模态模型识别、自动写入当前聚焦输入框。

WSL 环境可以完成前端构建、Rust 单元测试和业务逻辑验证；Windows 托盘、全局快捷键、麦克风录音、文本注入和安装包构建需要在 Windows 上验证。

## 快速开始

```bash
npm install
npm test -- --run
npm run build
cd src-tauri && cargo test
```

开发说明见 `docs/development.md`。

## 当前闭环

- 首页展示使用统计和最近识别记录。
- 配置页支持 MiMo-v2.5 和 Qwen3.5-Omni 模板、Base URL、模型、API Key 安全引用和快捷键。
- 数据页支持识别记录、自定义词库和风格提示词。
- WSL/浏览器预览下提供模拟识别，Tauri 环境下通过命令层接入 SQLite 和后端服务。

## GitHub Actions

- `CI`：在 Ubuntu 上验证前端测试、前端生产构建和 Rust 核心测试。
- `Release`：在 Windows runner 上构建 Tauri Windows 安装包，并在推送 `v*` tag 或手动触发时发布 GitHub Release。
