# 说文 saynow 项目全面评审与优化建议（2026-07）

> 评审范围：架构设计、产品功能合理性、UI 交互、视觉设计、工程质量。
> 基线版本：v0.1.37（main @ 0454431）。
> 每条建议标注优先级：**P0**（影响核心体验/安全，应尽快做）、**P1**(明显收益，近期排期）、**P2**（打磨项，机会性做）。

---

## 0. 总体评价

项目整体完成度高于典型 MVP：核心链路（全局热键 → 原生录音 → 多模态识别 → 焦点恢复 → 粘贴注入 → 纠错浮窗 → 自学习）已经闭环，Rust 端平台代码（键盘钩子、剪贴板、焦点恢复）细节处理认真，测试对 domain 层覆盖良好，文档（PRD/技术设计/自学习设计）齐备。

当前最值得投入的三个方向：

1. **可信与安全**：API Key 实际明文落库，与技术设计承诺不符；麦克风流常驻导致系统"麦克风使用中"指示灯常亮，隐私观感差。
2. **反馈闭环**：大量后端错误（保存校验失败、识别失败详情）在 UI 上无处显示；识别失败无重试；学习规则学错了无法停用。
3. **前端架构收敛**：App.vue 已经是 750 行的 God Component，样式体系两套并存互相覆盖，继续叠功能会迅速失控。

---

## 1. 架构设计

### 1.1 前端：App.vue 巨型组件（P0）

**问题**：`src/App.vue` 集中了页面路由、9 类数据的全局状态、录音会话状态机、热键监听重试、纠错事件、学习引擎空闲调度、overlay 窗口管理，约 750 行。所有数据靠 props 逐层下传、事件逐层上传（prop drilling），任何新功能都要改这一个文件。

**建议**：
- 按职责拆成 composables：`useRecordingSession()`、`useHotkeyMonitor()`、`useRecorderOverlay()`、`useLearningEngineScheduler()`、`useAppData()`（或引入 Pinia 存放 config/providers/records 等共享状态）。
- 页面组件直接从 store/composable 取数，App.vue 只留壳与装配。

### 1.2 `refreshAll()` 全量刷新反模式（P1）

**问题**：`src/App.vue:138` 的 `refreshAll()` 一次拉取 dashboard、config、providers、records、corrections、vocabulary、styles、preferences、learningEngineConfig、learningRules 共 10 个接口；而删除一个词条、保存一次纠错等任何小操作后都调用它。且多数写接口本已返回更新后的列表（如 `deleteVocabulary` 返回 `VocabularyItem[]`），先赋值再 refreshAll 是双重浪费（`src/App.vue:583-591`）。

**建议**：改为按域刷新——写哪个域就更新哪个域的状态；识别完成后只刷新 dashboard + records。10 个串行 `await` 也应改为 `Promise.all` 并行。

### 1.3 后端：同步阻塞的网络请求，不可取消、超时不可控（P0）

**问题**：`recognize_audio` / 学习引擎请求使用 `reqwest::blocking`（`src-tauri/src/commands.rs:544`、`commands.rs:232`），运行在 Tauri 同步命令线程上：
- 无显式超时（依赖 blocking client 默认值），API 挂起时前端 `busy` 状态长时间卡死，热键被 `busy` 抑制，用户"整个工具失灵"。
- 无法取消：用户说错了/不想等了没有任何手段中断请求。

**建议**：改为 `async fn` 命令 + `reqwest`（async）+ `tokio::time::timeout`（如 30s 可配置），持有 `CancellationToken`，暴露 `cancel_recognition` 命令；overlay 的 processing 态提供"取消"按钮或再按热键取消。

### 1.4 每次请求新建 HTTP Client（P2）

**问题**：`reqwest::blocking::Client::new()` 每次识别/学习都新建（`commands.rs:558`、`commands.rs:232`），无连接池/TLS 会话复用，冷启动延迟叠加在识别延迟上。

**建议**：Client 建一次放进 Tauri state（与 1.3 一起改造）。

### 1.5 录音数据在 IPC 上多绕两圈（P1）

**问题**：录音在 Rust 侧完成，却把 WAV → base64 → IPC 传给前端（`stop_native_recording`），前端原样再经 IPC 传回后端 `recognize_audio`。2 分钟 48kHz 双声道 PCM16 约 23MB，base64 后 ~30MB，双向 IPC 序列化 60MB，只为了在前端拼一个参数。

**建议**：新增后端命令 `stop_and_recognize()`：音频留在 Rust 侧直接进入识别管线，前端只收最终 `RecognitionRecord`。同时消除 `RecordedAudio` 大字符串在 JS 堆里的驻留。

### 1.6 音频不做降采样（P1）

**问题**：`audio.rs` 按设备默认配置录制（常见 48kHz / 2ch），未降采样。语音识别 16kHz 单声道即可，当前体积约为必要值的 6 倍——上传更慢、费用更高、部分 ASR 端点可能限制 body 大小。

**建议**：在 `encode_wav_pcm16` 前混合声道并线性重采样到 16kHz mono；顺带 `stop()` 里 `elapsed().as_secs()` 截断会把 1.9s 录音记成 1s，改用四舍五入，统计时长更准。

### 1.7 麦克风流常驻（warm stream）带来的隐私观感（P0）

**问题**：为降低启动延迟，`NativeAudioRecorder::warm_up()` 在应用启动即创建并 `play()` 输入流并常驻（`lib.rs:30`、`audio.rs:40`）。虽然不录音时丢弃数据，但 OS 层面麦克风被持续占用：Windows 任务栏/设置会显示"saynow 正在使用麦克风"，用户会认为软件在持续监听；同时默认输入设备热插拔后 warm stream 失效，无自动重建逻辑（`stop()` 拿不到样本只返回 `None`）。

**建议**：
- 改为"懒启动 + 延迟释放"：首次录音时创建流，空闲 N 秒（如 60s）后释放；或在设置中提供"低延迟模式（常驻麦克风）"开关并说明。
- 监听默认设备变化（WASAPI notification 或每次 start 前校验设备），流失效时自动重建。

### 1.8 API Key 存储与技术设计不符（P0）

**问题**：`docs/design/technical-design.md` 承诺"API Key 只保存安全存储引用，不在普通表中明文落库"。实际上：
- 配置页让用户直接粘贴 Key，`resolve_api_key`（`commands.rs:622`）把任意非前缀字符串按 literal 使用，**明文存进 SQLite**。
- `credential-manager:mimo` 并未对接 Windows 凭据管理器，只是读环境变量 `SAYNOW_MIMO_API_KEY` 的别名——模板默认值对普通用户完全不可用（不懂设置环境变量），是新用户配置失败的最大陷阱。

**建议**：接入真实的 Windows Credential Manager（`keyring` crate 即可），保存时写入凭据库、表中只存引用；UI 上 Key 输入框显示"已保存（••••1234）/未保存"状态。至少短期内：模板不要默认填 `credential-manager:*`，改为空值 + placeholder 说明。

### 1.9 app_config 与 provider_configs 双写同步（P1）

**问题**：`db.rs` 中启用中的 provider 字段会在 `get_config` 时覆盖进 `app_config`，`save_config` 又把 config 反向 upsert 回 `provider_configs`，`delete/select_provider_config` 还要手工同步一遍。双事实源，同步路径有 4 条，是历史 bug 的温床。

**建议**：单一事实源：`app_config` 只保留 `hotkey` 与 `active_provider_id`（外键），识别时 join 出启用的 provider。迁移成本低（已有 `add_column_if_missing` 机制）。

### 1.10 数据库无版本化迁移（P2）

**问题**：`migrate()` 依赖 `CREATE TABLE IF NOT EXISTS` + 逐列探测补列，且 `get_config` 热路径里永久保留 "F8 → Alt" 的历史修复（`db.rs:212`）。演进多了将难以追踪。

**建议**：加 `PRAGMA user_version` 版本号迁移链，一次性迁移脚本按版本执行；顺带为连接开 WAL 模式提升并发读写。

### 1.11 前后端 Prompt 逻辑重复且前端版已死代码（P1）

**问题**：`src/domain/promptContext.ts` 与 `src-tauri/src/prompt.rs` 是同一段 prompt 组装逻辑的两份拷贝（TS 版还缺 learning rules），且 **TS 版在产品代码中无任何调用，只有测试引用**。同类死代码还有 `provider.rs:6` 的 `ProviderConfig` 结构体（与 db.rs 同名结构体并存、从未使用）、`tauri.ts` 的 `restoreInputTarget` / `addVocabulary` / `refreshLearningRules` 前端从未调用。

**建议**：要么删除 TS 版，要么把它真正用起来（在个性化页做"当前提示词预览"功能，帮助用户理解词库/风格/规则如何影响识别——这是个低成本高价值的功能点）。清理其余死代码。

### 1.12 Provider 判别依赖字符串猜测（P1）

**问题**：`is_qwen_provider` 用 `contains("qwen")`，MiMo 鉴权头用 `base_url.contains("xiaomimimo.com")` 判断（`commands.rs:614`），模型名含 "-asr" 判 ASR payload。用户自定义 provider 名称/URL 稍有出入行为就悄悄变化，难以排查。

**建议**：`provider_configs` 增加显式 `kind` 字段（`openai-compatible | qwen-omni | mimo | mimo-asr`），模板写入 kind，payload/鉴权按 kind 分派；自定义 provider 默认 openai-compatible。

### 1.13 学习引擎调度在前端（P1）

**问题**：空闲触发定时器在主窗口 JS 里（`App.vue:635`），主窗口 webview 若被系统挂起则不触发；文档中"每天首次启动补偿整理"未实现；学习请求本身也是阻塞命令（见 1.3）。

**建议**：调度下沉到 Rust（tokio 定时任务 + 上次运行时间落库），前端只负责配置与展示。

### 1.14 单实例与开机自启缺失（P1）

**问题**：无单实例保护，双开会产生两个键盘钩子与两个托盘图标，行为异常；作为常驻托盘工具没有开机自启选项。

**建议**：接入 `tauri-plugin-single-instance`（二次启动时唤起已有主窗口）与 `tauri-plugin-autostart`（设置页加开关）。

### 1.15 Capability 未按窗口收敛（P2）

**问题**：`capabilities/default.json` 对 `main` 和 `recorder` 两个窗口开放同等权限，且所有自定义命令对两个窗口均可调用；recorder 浮窗理论上能调 `save_config`、`delete_provider_config` 等。本地应用风险低，但边界不清晰。

**建议**：拆分 capability：recorder 只保留事件与自身窗口控制所需命令。

---

## 2. 产品功能合理性

### 2.1 首次使用引导缺失（P0）

**问题**：安装后首屏是空数据看板，用户必须自行发现"要先去配置页添加供应商 + 理解 API Key 怎么填 + 设置热键"。配置错误只有在真的按键说话后才报错。结合 1.8 的 Key 陷阱，新用户首次成功率堪忧。

**建议**：首启动向导（3 步：选供应商模板并填 Key → 测试连接 → 试录一句话），完成前首页显示引导卡片而非空看板。

### 2.2 供应商无"测试连接"（P0）

**问题**：保存配置后无任何验证手段，错误的 base_url/Key/模型名要到实际识别时才暴露，且错误细节只落在数据页记录里。

**建议**：配置页加"测试"按钮：发一个最小请求（或 1 秒静音音频），就地显示成功/失败与失败原因。

### 2.3 识别失败无重试，音频即弃（P0）

**问题**：网络抖动、限流、Key 过期等失败后，用户唯一的选择是把话重新说一遍。音频数据在失败后直接丢弃。

**建议**：失败时保留最后一段音频（内存即可），overlay 的 error 态提供"重试"按钮；数据页失败记录同样提供重试。配合 1.3 的 async 化实现。

### 2.4 后端校验错误在 UI 上不可见（P0）

**问题**：多处后端返回的业务错误前端没有出口，静默失败：
- 保存学习引擎配置不完整时后端返回"请填写完整的 LLM 供应商…"（`commands.rs:410`），前端 `saveLearningEngineSettings` 只在 finally 里复位 saving，**错误消息被吞掉**（`App.vue:613`）。
- `saveProvider` 前端字段不全时静默 `return`（`ConfigPage.vue:65`），无任何提示。
- 识别失败 overlay 只显示"识别失败"1.2 秒，具体原因要去数据页翻。

**建议**：建立全局 toast/通知机制（一个轻量 composable 即可），所有 invoke 失败统一弹出可读错误；表单加内联校验提示；overlay error 态显示简短原因并延长可见时间/可点击查看。

### 2.5 学习规则不可管理，与设计文档脱节（P0）

**问题**：`docs/self-learning-engine.md` 定义了规则生命周期（observed/candidate/active/pinned/disabled）与"候选规则先影子评估、不影响输出"的原则。实际：
- prompt 组装直接采纳 `candidate`（`prompt.rs:49`），等于候选即生效，影子评估未实现；
- UI 上规则只读，**误学的规则无法停用/删除/固定**，会永久污染后续识别；
- 纠错记录也无法删除（隐私策略中"支持清空纠错历史和学习规则"未实现）。

**建议**（按序）：
1. 个性化页给每条规则加"停用/固定/删除"操作（DB 已有 status 字段，改动小）；
2. 加"清空纠错记录/清空学习规则"入口；
3. 中期再实现影子评估晋级 active，并把 prompt 采纳范围收敛到 active/pinned。

### 2.6 词库能力被 UI 阉割（P1）

**问题**：`vocabulary` 表设计了 alias（别名，对同音专名纠正价值最大）、category、note、enabled，UI 只支持批量加纯词条与删除。且：
- `add_vocabulary_terms` 不去重，同词重复插入产生重复行，prompt 中重复出现；
- prompt 只取最近 30 个词（`prompt.rs:23`），超出后旧词条静默失效，用户不知情。

**建议**：词条支持点击编辑 alias/enabled；插入时按 term 去重（表加 UNIQUE 或 upsert）；词库超过 30 条时在 UI 提示"识别时仅使用最近 30 条启用词"或改为按优先级选取。

### 2.7 纠错浮窗 4 秒超时太短（P1）

**问题**：`CORRECTION_PROMPT_TIMEOUT_MS = 4000`（`App.vue:104`）。用户注意力此刻在目标应用输入框上，4 秒内要注意到浮窗 → 移动鼠标 → 点"编辑"非常勉强，纠错入口的实际转化率会很低——而纠错对是自学习引擎唯一的强信号来源。

**建议**：延长到 6–8 秒且鼠标悬停时暂停倒计时；提供"纠正上一条"的全局热键（设计文档已提到此入口）；数据页历史记录支持事后纠错（同样是文档中规划的强信号入口）。

### 2.8 Alt 单键作为默认热键值得商榷（P1）

**问题**：默认热键为单独 Alt（`db.rs:12`）。Alt 是 Windows 交互核心键（菜单激活、Alt+Tab），为此 platform.rs 实现了复杂的捕获-重放机制（capture_events/captured_order replay），历史上已多次修复（近 5 个 commit 中 2 个与热键稳定性有关）。复杂度与误触风险都高。

**建议**：默认改为冲突面小的组合（如 `Ctrl+Win`、`F9`）；Alt 类热键保留为高级选项并在 UI 标注可能影响系统快捷键。同时考虑提供"按一下开始、再按结束"的 toggle 模式（长句输入按住 2 分钟并不舒适，watchdog 也正是 120s）。

### 2.9 录音过程无时长与音量反馈（P1）

**问题**：overlay 录音态只有呼吸动画：不显示已录制秒数（无从知道离 120s watchdog 强停还有多久），不显示音量电平（麦克风坏了/选错设备时，用户只能等识别结果为空才发现）。

**建议**：录音态显示计时；用现成的采样缓冲计算 RMS 电平驱动一个小波形/音量条（数据都在 `audio.rs` 的回调里，成本低）；接近 watchdog 上限前 10 秒给出视觉警告。

### 2.10 数据页只是只读列表（P1）

**问题**：仅时间轴展示最近 200 条：无搜索/筛选、无复制按钮、无删除、无导出、无分页（200 条以前的记录永远看不到但永久占库）、无"重新注入"、无事后纠错。识别文本永久明文留存但没有任何清理手段——也是隐私问题。

**建议**：按价值排序补齐：一键复制 → 搜索 → 删除/清空（含隐私说明）→ 事后纠错（联动 2.5/2.7）→ 导出 CSV/JSON → 分页或虚拟滚动。

### 2.11 统计维度太薄，弱于概念稿（P1）

**问题**：首页只有三个 lifetime 累计值（总时长/条数/字数）。`docs/assets/ui-concept.png` 概念稿里有今日时长、较昨日增减、准确率、待处理项——实现明显退化。作为"效率工具"，用户感知不到收益。

**建议**：加"今日/本周"维度与环比；估算节省时间（字数 ÷ 常人打字速度 − 说话时长）；识别成功率与平均识别延迟（`recognize_audio` 已记录 elapsed_ms，只差落库一个字段）；近 7 日用量小柱状图。

### 2.12 风格切换路径太深（P2）

**问题**：风格提示词一次只能启用一个（合理），但切换要主窗口 → 个性化 → 风格 Tab → 选中 → 勾选启用 → 保存。聊天/纪要/工单是高频切换场景。

**建议**：托盘菜单加"风格"子菜单快速切换；或热键循环切换 + overlay 短暂显示当前风格名。

### 2.13 学习引擎 localOnly 模式不可选（P2）

**问题**：类型与后端均支持 `localOnly`（本地启发式、零额外配置），但 UI 保存时硬编码 `runMode: 'llmAssist'`（`PersonalizationPage.vue:79`）。不愿为学习引擎单独配 LLM 的用户没有低门槛选项。

**建议**：UI 开放模式选择并说明差异；或如果决定 localOnly 只是内部调试路径，就从 types 和文案中隐藏，避免歧义。

### 2.14 撤销/替换机制的风险未告知（P2）

**问题**：撤销与替换实现为模拟 Ctrl+Z（+重新粘贴）。目标应用若不支持撤销、撤销粒度不同（终端、部分聊天框），或用户在浮窗停留期间又输入了内容，会误删用户数据。

**建议**：短期在纠错浮窗 hover 提示中说明机制与限制；中期可选择性对已知应用类别（终端类进程名）禁用替换只保存纠错。

### 2.15 常驻工具的基础设施缺失（P1）

- **更新检查**：无版本更新提示。建议轻量方案：启动时比对 GitHub Releases latest tag，关于页与托盘提示新版本（或直接接 tauri-plugin-updater）。
- **关闭到托盘无提示**：首次点 X 静默隐藏，Windows 用户预期是退出。建议首次隐藏时弹托盘气泡"已最小化到托盘，可在设置中更改"。
- **托盘无状态/操作**：录音/识别中托盘图标不变化；菜单只有显示/退出。建议：录音时切换托盘图标；菜单加"暂停热键监听""切换风格"。
- **日志入口**：`get_runtime_log_path` 命令已存在但 UI 未使用。建议关于页加"打开日志目录"，便于用户反馈问题时取日志。

### 2.16 中文硬编码的输出语言假设（P2）

**问题**：默认风格与整理规则均假设"简体中文输出"，用户说英文或中英混说时行为未定义（可能被翻译）。

**建议**：设置里加输出语言选项（跟随语音/中文/英文），注入 prompt 对应约束。

---

## 3. UI 交互

### 3.1 操作反馈缺失（P0，与 2.4 同源）

保存成功无 toast；删除供应商/词条/风格**无确认对话框且不可撤销**，而删除按钮与保存按钮相邻（`ConfigPage.vue:217-231`），误触成本高。建议：统一 toast + 危险操作二次确认（或 5 秒可撤销式删除）。

### 3.2 图标按钮过度使用（P1）

配置页与个性化页的保存/启用/删除/整理全部是无文字圆形图标按钮，语义靠 hover title；"立即整理"和"保存中"共用同一个 activity 图标，含义冲突。建议：主操作恢复"图标+文字"，仅列表行内操作用纯图标；saving 态换 spinner。

### 3.3 风格列表的开关是"假开关"（P1）

**问题**：`.style-item` 行内渲染 iOS 风格开关（`PersonalizationPage.vue:178`），视觉上强烈暗示可点击切换，实际点击整行只是"选中进入编辑"，真正切换启用要在右侧表单勾 checkbox 再保存。视觉暗示与行为严重不一致。

**建议**：让行内开关真正可点（stop propagation，点击即切换启用并保存）；或去掉开关改为"启用中"徽标。

### 3.4 列表入场动画随条数线性放大延迟（P1）

**问题**：`animationDelay: index * 50ms`（DataPage）/ `index * 40ms`（HomePage），200 条记录时最后一条要 10 秒后才出现，滚动到底部是空白。

**建议**：`Math.min(index, 12) * 40ms` 封顶，或只对首屏可见项做 stagger。

### 3.5 热键录制交互不自解释（P1）

录制区自身可聚焦按键但必须先点键盘小图标才进入录制态；trash 清空后保存即"禁用热键"但无说明；Esc 取消、15 秒超时自动退出均无 UI 提示；修饰键单独作为热键需要"按下后松开"才确认（`finishModifierOnlyHotkey`），过程无引导文案。建议：录制态显示辅助文案（"按组合键，Esc 取消；松开以确认单修饰键"），清空后显示"热键已禁用"状态徽标。

### 3.6 overlay 遮挡与不可移动（P1）

浮条固定在屏幕底部居中，恰好是聊天软件输入框的典型位置。建议：支持拖动并记住位置（相对工作区），或提供顶部/底部位置选项；处理与任务栏自动隐藏的边缘情况。

### 3.7 无障碍与键盘可达性（P2）

- ios-switch 为纯 div，无 role/键盘支持（feature-toggle-row 用了 button+aria-pressed，是对的，风格列表行没有）；
- 纠错浮窗按钮仅鼠标可达（no-activate 窗口特性所限，但"编辑"入口可以考虑热键触达，见 2.7）；
- 主界面未管理 tab 顺序与 focus 样式（多处 `outline: none` 抹掉了焦点环）。

建议：开关组件统一为带 role="switch" 的真按钮；保留 :focus-visible 焦点环。

### 3.8 首次加载与空态（P2）

dashboard 为 null 时显示 0 而非骨架屏/加载态；空态文案不错但缺行动按钮（如数据页空态给"去配置供应商"）。建议空态附带 CTA，加载中显示 skeleton。

### 3.9 流式转写展示受限（P2）

后端已实现流式转写事件（`recorder-transcript`，很好的功能），但 overlay 单行 12px ellipsis 展示，长句只能看到开头。建议：录音/识别态允许浮条纵向扩展 2–3 行，或滚动显示尾部（用户更关心最新识别出的词）。

---

## 4. 视觉设计

### 4.1 两套设计语言并存（P0，视觉层最大问题）

**问题**：`styles.css` 定义了一套玻璃拟态体系（backdrop-filter、玻璃描边、渐变卡），而 HomePage/ConfigPage/PersonalizationPage 各自用 `:deep()` 大面积覆盖成纯白扁平卡（页内注释直言"替换原 styles.css 依赖"）。结果：
- 首页 metric 卡是纯白、记录卡是半透明玻璃，数据页又是另一种白卡+时间轴，风格漂移；
- `primary-button/secondary-button/danger-button` 在 styles.css、ConfigPage、PersonalizationPage 三处重复定义且参数略有出入（高度 40 vs 38，圆角 12 等）；
- `slideUpFade/fadeSlideUp/fadeUp` 三个几乎相同的 keyframes 重复定义。

**建议**：做一次样式收敛专项——
1. 确定基调（建议顺势采用页面覆盖后的"纯白扁平 + 轻投影"，删除玻璃拟态残留）；
2. 按钮/输入框/卡片/开关沉淀为全局组件或全局 class，删除页面级重复定义；
3. 动画统一为一个 keyframes + 工具 class。

### 4.2 色彩硬编码泛滥（P1）

`#0f8f83 / #08776f / #1d1d1f / #86868b / rgba(15,143,131,…)` 等在各 scoped 样式中出现数十次，`--accent` 等变量形同虚设。换主题色需全局改几十处，暗色模式无从做起。建议：全部替换为 CSS 变量引用，扩充 token 集（文本三级、边框两级、语义色）。

### 4.3 暗色模式缺失（P2，但依赖 4.2）

常驻桌面工具不跟随系统深浅色，夜间纯白刺眼；overlay 白条在深色应用/壁纸上也突兀。建议：完成 4.2 后加 `prefers-color-scheme: dark` 变量覆盖 + 设置页手动开关；overlay 单独适配半透明深色底。

### 4.4 overlay 在白色背景上"隐形"（P1）

**问题**：浮条 `background: #ffffff` 且 `border: transparent、box-shadow: none`（`RecorderOverlay.vue:230-234`），叠在 Word/记事本等白底应用上完全没有边界，只剩一个小圆点动画。

**建议**：加轻投影（如 `0 4px 24px rgba(0,0,0,.12)`）与 1px 中性描边；这也是与 4.3 一致的"浮层应自带对比度"原则。

### 4.5 字体字重体系（P2）

字重用了 400/500/600/650/700/750/800/850/900 九档，中文字体（YaHei）不支持如此多实梯度，多为浏览器伪加粗，观感发糊。字号 58/48/32/30/22/20/18/16/15/14/13/12/11 无节制。建议：收敛为 4 档字重（400/500/600/700）与 6 级字号的 type scale，写进 token。

### 4.6 布局密度与信息量（P2）

1180×760 的首页只有 3 张大卡 + 一个列表，58px 大数字略空洞；metric 卡 min-height 190px。配合 2.11 的统计增强，把趋势 sparkline 放进卡片，密度与价值同时提升。

### 4.7 死断点（P2）

窗口 `minWidth: 960`，而 styles.css / 各页面存在 `max-width: 860/900px` 的媒体查询分支（侧栏折叠为图标栏等）永远不会触发，是维护噪音。建议：删除 <960 的分支，或把窗口 minWidth 降到允许紧凑布局的值并真正测试折叠形态。

### 4.8 品牌细节（P2）

品牌字"语"与产品名"说文/saynow"的关系略拧（建议统一为"说"或做个简单 logotype）；关于页二维码 `background-size: cover` 对非正方形源图会裁切，且图片缺失时是空虚线框，建议 `contain` + 缺省文案。

---

## 5. 工程质量与 DevOps

### 5.1 前端核心状态机零测试（P1）

domain 纯函数有测试，但最容易出错的部分——`RecordingSession` 竞态（按下未就绪即松开、start 失败后 release、双事件乱序）、纠错浮窗超时与编辑流转——完全无测试。这些逻辑一旦从 App.vue 抽成 composable（见 1.1），即可用 vitest 直接覆盖，两件事应一起做。

### 5.2 发版流程手工同步 5 处版本号（P1）

AGENTS.md 要求发版前人工同步 `package.json / package-lock.json / Cargo.toml / Cargo.lock / tauri.conf.json` 并手改 `release.yml` 中硬编码的 `releaseName: "saynow v0.1.37"` 与 changelog——极易漏改（workflow 里的版本号与 tag 不一致会发出错误标题的 release）。

**建议**：`releaseName: "saynow ${{ github.ref_name }}"`；release body 改由 tag message 或 CHANGELOG.md 生成；写一个 `npm run bump <version>` 脚本一次性改五个文件。

### 5.3 CI 在每次 push 都构建 Windows 安装包（P2）

`ci.yml` 的 build-windows 任务每次 push main 都跑完整 bundle（10 分钟级）。建议仅在 PR 打标签或 release 分支/tag 时构建，main push 只跑测试与 `cargo check`。

### 5.4 运行日志无上限（P1）

`saynow-runtime.log` 只追加不轮转，且前端 debugLog 把每次按键状态、每次 overlay 显隐都各自 invoke 一次写盘。常驻数月的工具日志会涨到数百 MB，高频 IPC 也有开销。建议：按大小轮转（如 5MB×3 份）；前端日志加级别开关（默认 info 以上），或批量缓冲后一次 IPC。

### 5.5 Lint/格式化缺位（P2）

无 eslint/prettier/clippy/rustfmt 的配置与 CI 步骤。多人协作或 AI 协作场景下风格漂移会加速（当前样式重复问题即是例证）。建议：加 eslint+prettier（vue/ts 预设）、CI 加 `cargo clippy -- -D warnings` 与 `cargo fmt --check`。

### 5.6 杂项（P2）

- `insert_failed_record` 中 `let _ = latest_record(db)?;` 若查询失败会用查询错误替换真正的识别错误，误导排查；
- `open_external_url` 用 `cmd /C start` 拉浏览器，URL 白名单硬编码在 Rust 中，扩链接要发版——建议换 `tauri-plugin-opener` + 域名白名单；
- `commands.rs` 中 `eprintln!` 与 `runtime_log::write_line` 双轨并行，建议统一一个带级别的日志函数；
- README"历史与统计：查看…输入效率数据"表述超前于实现（见 2.11），文档与实现对齐。

---

## 6. 优先级汇总

### P0 —— 可信、安全与核心反馈闭环

| # | 事项 | 章节 |
|---|------|------|
| 1 | API Key 接入真实凭据管理器，停止明文落库；模板默认值去掉环境变量引用 | 1.8 |
| 2 | 麦克风流改为懒启动+延迟释放，消除"常驻占用麦克风"观感 | 1.7 |
| 3 | 识别/学习请求 async 化：显式超时、可取消、失败可重试 | 1.3 / 2.3 |
| 4 | 全局 toast：吞掉的后端错误全部可见；危险删除加确认 | 2.4 / 3.1 |
| 5 | 学习规则可停用/删除/固定；纠错与历史可清空 | 2.5 |
| 6 | 首启动向导 + 供应商"测试连接" | 2.1 / 2.2 |
| 7 | App.vue 拆分 composables（为以上改动铺路） | 1.1 |
| 8 | 视觉基调二选一，删除双设计语言 | 4.1 |

### P1 —— 明显收益

按域刷新替代 refreshAll（1.2）｜音频留后端+降采样 16k mono（1.5/1.6）｜config/provider 单一事实源（1.9）｜prompt 双份代码处置+提示词预览功能（1.11）｜provider kind 显式化（1.12）｜学习调度下沉 Rust（1.13）｜单实例+自启动（1.14）｜词库 alias/去重/30 条上限提示（2.6)｜纠错窗口延时+悬停暂停+历史页纠错（2.7）｜默认热键调整与 toggle 模式（2.8）｜录音计时+音量反馈（2.9）｜数据页复制/搜索/删除（2.10）｜统计增强（2.11）｜更新检查+托盘增强+关闭提示（2.15）｜假开关修复（3.3）｜动画延迟封顶（3.4）｜热键录制引导文案（3.5）｜overlay 可移动（3.6）｜overlay 加投影描边（4.4）｜色彩 token 化（4.2）｜录音状态机测试（5.1）｜发版脚本化（5.2）｜日志轮转（5.4）

### P2 —— 打磨

HTTP client 复用（1.4）｜DB 版本化迁移+WAL（1.10）｜capability 拆分（1.15）｜风格快速切换（2.12）｜localOnly 开放或移除（2.13）｜撤销风险告知（2.14）｜输出语言选项（2.16）｜无障碍（3.7）｜骨架屏与空态 CTA（3.8）｜流式转写多行展示（3.9）｜暗色模式（4.3）｜字体体系（4.5）｜信息密度（4.6）｜死断点清理（4.7）｜品牌细节（4.8）｜CI 构建策略（5.3）｜Lint 基建（5.5）｜杂项（5.6）

---

## 7. 建议的执行顺序（供参考）

1. **第一批（安全与反馈）**：P0 #1–#5。这批做完，产品从"能用"变成"敢长期用"。
2. **第二批（新用户转化）**：P0 #6 + 词库/纠错入口增强（2.6/2.7）+ 统计增强（2.11）。降低首次配置流失，强化"越用越像你"的核心卖点。
3. **第三批（架构与视觉收敛）**：P0 #7、#8 + P1 中的 1.x 架构项与 4.x 视觉项。为后续功能腾出干净地基。
4. **持续**：P2 项随迭代机会性处理。
