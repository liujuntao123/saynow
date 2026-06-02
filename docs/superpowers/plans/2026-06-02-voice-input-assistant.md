# saynow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete MVP Windows desktop voice recognition assistant that records speech by global hotkey, calls online multimodal model APIs, stores local user data, and inserts recognition results into the active input window.

**Architecture:** Use Tauri 2 as the desktop shell, Vue 3 for the UI, Rust for platform integration and command handlers, and SQLite for local persistence. Windows-only behaviors are isolated behind a platform adapter; in WSL/Linux they return explicit unsupported statuses while keeping the app buildable and testable.

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Vite, Rust, SQLite via `rusqlite`, Vitest, Cargo tests.

---

## File Structure

- `package.json`: frontend scripts and Tauri commands.
- `src/`: Vue app, UI components, pages, API bridge, and frontend domain helpers.
- `src-tauri/`: Rust Tauri app, command handlers, persistence, provider adapters, prompt context builder, platform adapter.
- `docs/design/`: PRD and technical design docs used by implementers.
- `docs/assets/ui-concept.png`: optional generated UI reference.

## Execution Tasks

### Task 1: Scaffold Project And Documentation

**Files:**
- Create: `package.json`, `index.html`, `src/main.ts`, `src/App.vue`, `src/styles.css`
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`
- Create: `docs/design/prd.md`, `docs/design/technical-design.md`

- [x] Create minimal Tauri 2 + Vue 3 structure manually instead of relying on an interactive scaffold.
- [x] Write concise PRD and technical design docs into `docs/design`.
- [x] Add scripts: `dev`, `build`, `test`, `tauri`.

### Task 2: Frontend Domain Tests And Helpers

**Files:**
- Create: `src/domain/stats.ts`
- Create: `src/domain/promptContext.ts`
- Create: `src/domain/__tests__/stats.test.ts`
- Create: `src/domain/__tests__/promptContext.test.ts`

- [x] Write failing tests for usage stat aggregation.
- [x] Implement stat aggregation.
- [x] Write failing tests for prompt-context preview assembly.
- [x] Implement prompt-context preview assembly.
- [x] Run `npm test -- --run`.

### Task 3: Rust Core Tests And Services

**Files:**
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/stats.rs`
- Create: `src-tauri/src/prompt.rs`
- Create: `src-tauri/src/provider.rs`
- Create: `src-tauri/src/platform.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] Write failing Cargo tests for stat aggregation.
- [x] Implement Rust stat aggregation.
- [x] Write failing Cargo tests for prompt context.
- [x] Implement prompt context builder.
- [x] Write failing Cargo tests for OpenAI-compatible request/response parsing.
- [x] Implement provider request/response helpers.
- [x] Add WSL/Linux platform adapter returning unsupported for Windows-only actions.
- [x] Run `cargo test`.

### Task 4: Persistence And Tauri Commands

**Files:**
- Create: `src-tauri/src/db.rs`
- Create: `src/api/tauri.ts`
- Modify: `src-tauri/src/lib.rs`

- [x] Implement SQLite schema for app config, providers, records, vocabulary, and style prompts.
- [x] Implement Tauri commands for reading dashboard data, listing records, saving config, managing vocabulary and style prompts, and running a simulated recognition cycle.
- [x] Keep API keys represented as secure-store references in schema; do not persist plaintext keys in normal tables.
- [x] Run `cargo test`.

### Task 5: Complete Vue UI

**Files:**
- Modify: `src/App.vue`, `src/styles.css`
- Create: `src/pages/HomePage.vue`, `src/pages/ConfigPage.vue`, `src/pages/DataPage.vue`, `src/pages/FeedbackPage.vue`
- Create: `src/components/AppShell.vue`, `src/components/MetricCard.vue`, `src/components/StatusBadge.vue`
- Create: `src/types.ts`

- [x] Build left-navigation shell with four pages.
- [x] Build homepage dashboard and recent recognition records.
- [x] Build config page for provider templates and hotkey configuration.
- [x] Build data page with records, custom vocabulary, and style prompt tabs.
- [x] Build feedback placeholder page.
- [x] Add polished restrained desktop styling.

### Task 6: Verification And WSL Skip Notes

**Files:**
- Create: `docs/development.md`
- Modify: `README.md`

- [x] Document local commands, WSL limitations, Windows verification checklist, and provider configuration notes.
- [x] Run `npm test -- --run`.
- [x] Run `npm run build`.
- [x] Run `cargo test` in `src-tauri`.
- [x] Skip Windows-only Tauri bundling and live platform verification in WSL, and document the skip.

## Self Review

- PRD scope maps to UI pages, provider config, hotkey config, data pages, tray/hotkey/insert platform abstractions, and recognition records.
- Windows-only functions are isolated and do not block WSL builds.
- Provider adapters are MVP-ready for OpenAI-compatible APIs while leaving model-specific audio payload verification to Windows/API-account validation.
- No cloud sync, realtime captions, or batch audio import are included.
