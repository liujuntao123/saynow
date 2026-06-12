# 自学习引擎设计草案

## 目标

自学习引擎把用户的高质量纠错记录整理为小而稳定的个性化识别偏好，用于后续识别请求上下文和识别后处理。它不直接把完整历史塞进 prompt，也不把弱观测行为直接当成学习数据。

第一阶段只使用明确纠错对：

- 原始识别文本 `rawText`
- 用户确认后的 `correctedText`
- 纠错来源 `source`
- 是否成功替换回目标输入框 `applied`

## 数据分层

### 强信号

强信号可以进入学习队列：

- 用户通过“刚才输入”浮层编辑并确认。
- 用户通过未来的“纠正上一条”快捷键确认。
- 用户在专门的历史记录页手动修正。

这些数据一定能形成 `rawText -> correctedText`，适合做 diff、归因和规则提取。

### 弱信号

弱信号只用于调整交互策略，不直接生成学习规则：

- 上屏后短时间内频繁 Backspace/Delete。
- 用户立刻撤销刚才插入。
- 同一个 App 中纠错入口打开率高。
- 某类文本识别后被放弃的比例高。

弱信号可以影响“是否默认显示确认框”“纠错入口停留多久”“某些 App 是否进入保守模式”。

## 整理时机

自学习不应阻塞语音输入主链路。建议使用后台批处理：

1. 每新增 5 条有效纠错记录后触发一次轻量整理。
2. 应用空闲 30 秒后，如果有未处理纠错，触发整理。
3. 每天首次启动时，对最近未处理记录做一次补偿整理。
4. 用户在设置页点击“整理个性化规则”时手动触发。

整理任务要有去重和节流：

- 同一批纠错只处理一次。
- 单次最多处理最近 20 条未处理纠错。
- 超过 30 天或已提炼为稳定规则的原始纠错可只保留摘要。

## 大模型输入

输入给大模型的是结构化批次，而不是整库历史：

```json
{
  "locale": "zh-CN",
  "task": "extract_personal_speech_correction_rules",
  "existingRules": [
    {
      "id": 12,
      "type": "numeric_context",
      "pattern": "status|code|id 附近的一/二/三",
      "replacement": "1/2/3",
      "confidence": 0.82
    }
  ],
  "corrections": [
    {
      "id": 101,
      "rawText": "status不应该会是一吧",
      "correctedText": "status不应该会是1吧",
      "source": "post-insert-overlay"
    }
  ]
}
```

必要时可附加很少量上下文：

- 当前启用词库
- 当前风格提示词名称
- 最近活跃 App 分类，如 `code-editor`、`browser`、`chat`

不建议附加完整识别历史、完整窗口标题或敏感上下文。

## 大模型输出

模型必须输出可验证的 JSON：

```json
{
  "rules": [
    {
      "type": "numeric_context",
      "description": "在 status、code、id 等技术上下文附近，中文数字更可能表示阿拉伯数字。",
      "matchHints": ["status", "code", "id"],
      "from": ["一", "二", "三"],
      "to": ["1", "2", "3"],
      "confidence": 0.78,
      "evidenceCorrectionIds": [101],
      "risk": "medium"
    }
  ],
  "vocabularyCandidates": [
    {
      "term": "status",
      "aliases": ["状态"],
      "category": "code",
      "confidence": 0.86,
      "evidenceCorrectionIds": [101]
    }
  ],
  "ignored": [
    {
      "correctionId": 102,
      "reason": "用户重写了句子，不是识别纠错"
    }
  ]
}
```

输出进入本地候选表，不立即全部生效。

## 规则生命周期

每条规则有状态：

- `observed`：只出现一次，暂不应用。
- `candidate`：多次出现，进入影子评估。
- `active`：通过评估，进入 prompt 或后处理。
- `pinned`：用户明确确认，长期保留。
- `disabled`：用户撤销或误伤，停止使用。

晋级建议：

- 同类纠错出现 2 次进入 `candidate`。
- 影子评估命中率高且误伤低，进入 `active`。
- 用户在设置页确认，进入 `pinned`。

## 应用策略

优先级从高到低：

1. 固定词库和用户手动添加词条。
2. `pinned` 自学习规则。
3. `active` 自学习规则。
4. 最近高置信候选规则。

规则有两种应用位置：

- 请求上下文：适合词汇偏好、专有名词、风格倾向。
- 识别后处理：适合确定性格式规则，如技术上下文中的数字、符号归一。

不要把高风险替换做成无条件后处理。例如“是一 -> 是1”只能在代码、参数、状态值附近应用。

## 影子评估

候选规则先不影响用户输出，而是在后台模拟：

- 用规则处理历史 `rawText`。
- 比较是否更接近 `correctedText`。
- 统计可能误伤的历史成功记录。
- 如果规则需要上下文，检查上下文命中是否足够明确。

只有通过影子评估的规则才进入 `active`。

## 隐私策略

- 原始纠错记录默认只保存在本地。
- 请求模型整理前，尽量只发送最小片段和 diff。
- 支持关闭自学习。
- 支持清空纠错历史和学习规则。
- 长期保留规则摘要，短期保留原始纠错。

## 第一版落地范围

本次实现只完成纠错入口和纠错记录采集：

- 识别成功后直接上屏。
- 短时间展示“编辑/撤销”轻入口。
- 用户编辑确认后保存纠错对。
- 尝试撤销上一条插入并粘贴修正文案。

当前新增的自学习 v0 只保存学习引擎配置和候选规则落点，不影响识别输出：

- `correction_records` 保存明确纠错对。
- `learning_rules` 保存规则候选。
- `learning_engine_config` 保存独立 LLM 配置。
- 用户可在个性化页配置学习引擎供应商、URL、模型、API Key、触发条数和空闲时间。
- 默认不启用学习引擎，不请求学习模型。
- `llmAssist` 是主路径：后续由大模型整理纠错记录。
- `localOnly` 只是可选降级/调试模式，会使用本地启发式生成候选规则。

后续再新增：

- 更精细的 diff 提取和隐私裁剪。
- 规则审阅页，目前只能通过日志和数据库查看 learning_rules。
- 影子评估任务，把 `candidate` 晋级为 `active`。
- 设置页中的规则审阅和开关。

## 当前实现行为

当前学习闭环已经接通：

1. 用户通过纠错浮窗保存 `rawText -> correctedText`。
2. 后端写入 `correction_records`。
3. 如果学习引擎启用，前端按 `idleSeconds` 启动空闲定时器。
4. 空闲定时器触发后，后端检查未处理纠错条数是否达到 `minNewCorrections`。
5. 个性化页的“立即整理”按钮会强制运行一次学习任务，跳过触发条数限制。
6. `llmAssist` 模式会请求配置的 OpenAI-compatible Chat Completions 模型。
7. 模型返回 JSON 规则，后端解析后写入 `learning_rules`。
8. 已参与整理的纠错会写入 `learning_processed_at`，避免重复处理。
9. 后续语音识别会读取 `candidate / active / pinned` 且非 high-risk 的规则，拼入 prompt 的“用户个性化识别偏好”部分。

日志前缀统一为 `[saynow] learning engine ...`，用于调试请求触发、批次大小、响应长度和规则落库情况。
