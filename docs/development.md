# 开发说明

## 本地命令

```bash
npm install
npm test -- --run
npm run build
cd src-tauri && cargo test
```

## WSL 限制

当前项目是在 WSL 环境中开发。以下能力依赖 Windows 桌面环境，WSL 中自动跳过真实调用：

- 系统托盘
- 全局快捷键
- 麦克风录音
- 当前聚焦窗口捕获
- 文本自动写入
- Windows 安装包构建

WSL 中可以验证前端界面、领域逻辑、SQLite 持久化、Provider payload/response 处理和模拟识别闭环。

## Windows 验证清单

1. 在 Windows 原生环境安装 Rust、Node.js 和 Tauri 依赖。
2. 运行 `npm install`。
3. 运行 `npm run tauri dev -- --features desktop`。
4. 验证关闭窗口后托盘常驻。
5. 验证全局快捷键按下开始录音、松开结束录音。
6. 验证 MiMo-v2.5 和 Qwen3.5-Omni 的真实 API 音频入参格式。
7. 验证识别结果写入记事本、浏览器输入框、聊天软件输入框。
8. 验证写入失败时降级为复制到剪贴板。

## Provider 配置

MVP 内置两个模板：

- MiMo-v2.5：`https://api.mimo-v2.com/v1`
- Qwen3.5-Omni：`https://dashscope.aliyuncs.com/compatible-mode/v1`

模型调用以 OpenAI-compatible Chat Completions 为基础。不同供应商对音频字段可能有细微差异，正式联调时应在 `src-tauri/src/provider.rs` 中新增供应商级 payload 适配。
