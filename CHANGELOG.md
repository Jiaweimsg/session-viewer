# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.5.9] - 2026-04-27

### Added

#### 重置对话上报状态
- 设置页"高级"区块新增"重置对话上报状态"按钮（带二次确认）
- 删除 `conversation-state.json` 后下一轮 cycle 会重新 fresh scan 全部历史 jsonl
- 用于排错：当 dashboard 看不到对话内容、但用量正常时（typically state offset 已被推进到 EOF 但服务端那边数据缺失）
- 新增 Tauri 命令 `reset_conversation_state`

### Server-side

#### `/api/conversations` uuid 去重
- POST 处理在写入前先扫描目标 jsonl 中已存在的 uuid 集合，跳过重复 message
- 配合客户端"重置上报状态"功能：重传全量历史不会在服务端造成重复 NDJSON 行
- 没有 uuid 的消息（极少数 Codex 合成 ID 缺失场景）依然写入
- 响应字段新增 `skipped` 计数

## [0.5.8] - 2026-04-27

### Added

#### 系统托盘 + 后台运行
- 启动后在系统托盘显示图标，菜单：显示主窗口 / 立即上报 / 退出
- 关闭主窗口不退出进程，最小化到托盘；上报循环继续在后台跑
- 左键单击托盘图标重新打开窗口；只有从托盘菜单选"退出"才彻底结束进程
- macOS：Cmd+Q 仍可正常退出
- 启用 `tauri` crate 的 `tray-icon` feature

#### 开机自启（默认开启）
- 集成 `tauri-plugin-autostart` 2.5.1（macOS 用 LaunchAgent，Windows 用 Run 注册表）
- 应用首次启动时自动 enable，登录后无需手动开窗
- 不在设置页提供开关，避免误关导致后台上报中断

#### 用户身份订正
- 设置页新增"用户身份"区块：显示当前生效的姓名/邮箱及来源（Git / OS fallback）
- 允许手动覆盖姓名和邮箱，留空则回退到默认；保存后下一轮上报立即生效
- 服务端 `upsertUser` 在邮箱不变时会更新姓名，dashboard 自动同步
- 持久化到 `{state_dir}/identity-override.json`
- 新增模块 `src-tauri/src/identity.rs` 与 Tauri 命令 `get_identity_view` / `get_identity_override` / `set_identity_override`

### Fixed

#### Windows 兼容
- 周期性 git 子进程闪现 cmd 黑窗：`report::git_config` 在 Windows 上加 `CREATE_NO_WINDOW (0x0800_0000)` 标志
- git.exe 启动失败弹"应用程序无法正常启动 (0xC0000142)"系统对话框：进程启动时调 `SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX)`，子进程继承不再弹窗
- 身份信息（user_name / user_email / machine_id）在进程内 `OnceLock` 缓存，git 子进程每个 key 只 spawn 一次
- conversation `project` 字段按 `/` 与 `\` 双分隔符 rsplit，Windows cwd 不再退化为整个长路径

### Added (cont.)

#### 上报循环诊断日志
- 新增 `{state_dir}/session-viewer/conversation-cycle.log`，每轮 conversation 上传的 start / scanned / blocklist / batch 结果 / cycle end 都落盘
- 解决 Windows release build 无 console、无法定位 `eprintln!` 错误的问题
- 与 `conversation-errors.log`（4xx dead-letter）互补

## [0.5.7] - 2026-04-27

### Fixed

#### Windows 兼容
- 周期性 git 子进程闪现 cmd 黑窗：`report::git_config` 在 Windows 上加 `CREATE_NO_WINDOW (0x0800_0000)` 标志，5 分钟一次的身份采集不再弹窗
- conversation `project` 字段提取按 `/` 切割，Windows 上 cwd 是 `C:\…` 时退化为整个长路径：改为同时按 `/` 与 `\` rsplit

### Added

#### 上报循环诊断日志
- 新增 `{state_dir}/session-viewer/conversation-cycle.log`，每轮 conversation 上传的 start / scanned / blocklist / batch 结果 / cycle end 都落盘
- 解决 Windows release build 无 console、无法定位 `eprintln!` 错误的问题
- 与 `conversation-errors.log`（4xx dead-letter）互补

## [0.5.6] - 2026-04-27

### Added

#### 上报黑名单
- 新增 `src-tauri/src/blocklist.rs` 模块：以 `cwd` 前缀匹配排除指定目录的对话内容上报
- 黑名单仅作用于 `/api/conversations`（用户 Prompt 上传）；`/api/report`（用量统计）不受影响
- 持久化到 `{data_dir}/session-viewer/upload-blocklist.json`，每轮上传时 reload，编辑即生效
- 命中黑名单的消息仍推进 file offset / cursor mark，避免重复扫描；移除黑名单后这部分历史不补传
- 跨平台路径标准化：`\` 自动归一为 `/`，前缀比较防止 `/foo` 误命中 `/foobar`
- 新增 Tauri 命令 `get_upload_blocklist` / `set_upload_blocklist`

#### 设置页
- 新增 `/settings` 路由与侧边栏入口
- 整合"上报服务端地址"配置（原嵌在统计页的扳手弹窗）
- 整合"立即上报"按钮（原嵌在统计页右上角）
- 新增"对话内容上报黑名单"管理：列表 + 手动输入 + 文件夹选择器（`tauri-plugin-dialog`）

### Changed

- 统计页右上角移除"上报"按钮和扳手图标，仅保留月份筛选

## [0.5.5] - 2026-04-22

### Added

#### Cursor Agent Transcripts 接入
- 新增 `src-tauri/src/cursor/parser/agent_transcripts.rs` 读取 `~/.cursor/projects/**/agent-transcripts/**/*.jsonl` —— Cursor 新版 Agent 对话格式
- `scan_one_transcript`: 按字节 offset 增量扫描，与 Claude / Codex 的水位线一致
- 过滤规则：只保留 `<user_query>...</user_query>` 包裹的真实用户提问，排除 `user_info` / `git_status` / `rules` 等系统注入
- 合成 uuid = `{session_uuid}_{line_start_offset}`（无原生 uuid）
- timestamp 使用文件 mtime（transcripts 没有 per-message 时间戳）
- model = null（transcripts 没有 model 字段）

#### 双路数据源合并
- `conversation/cursor_scanner.rs::scan_all` 合并两条 Cursor 数据源：
  - 旧 SQLite bubble 路径（`scan_composers`，保留作老数据兜底）
  - 新 agent-transcripts 路径（`scan_transcripts`，byte-offset 水位线）
- `conversation/uploader.rs::flush` 对 cursor 工具同时调用 `advance_marks`（bubble 合成路径）+ `advance_state`（transcripts 真路径）
- `advance_state` 跳过 `cursor:...` 前缀的合成路径，避免污染 `file_offsets`

#### 本地 Cursor Stats 整合
- `cursor/commands/stats.rs` 在 bubble 聚合之后加入 transcripts 聚合：
  - `total_messages` / `daily_messages` / `daily_sessions` / `project_stats` 同步累加
  - `total_sessions += transcript_session_count`
  - Transcripts 无 token 数据，不进 `daily_tokens`

## [0.5.4] - 2026-04-22

### Changed
- 简化强制更新流程：放弃完整的 Tauri updater 方案（免去签名密钥管理与 CI 集成）。ForceUpdateOverlay 的"立即更新"按钮现在只是用系统浏览器打开 GitHub Releases 页面 (`https://github.com/Jiaweimsg/session-viewer/releases`)，由用户手动下载并安装新版本
- 版本检查改为每个 metrics cycle（5 分钟）复跑一次，边沿触发 `force-update` / `force-update-cleared` 事件；fail-open 时保持原状态
- 回退了 Phase N 期间引入的 `tauri-plugin-updater` / `@tauri-apps/plugin-updater` 依赖、`src-tauri/src/updater.rs` stub、`tauri.conf.json` updater 配置、capabilities 里的 updater 权限、服务端 `/api/updater/manifest.json` 路由与 `/releases/*` 静态托管、auth 白名单相关条目、以及 release.yml 里的签名 env 变量

## [0.5.3] - 2026-04-22

### Added

#### 服务端驱动的最低版本强制更新
- 服务端新增 SQLite `settings` 表 + `GET /api/config` (公开) + `PUT /api/config` (JWT) 路由
- Dashboard 头部新增"设置"按钮 → 弹窗配置 `min_client_version`（semver X.Y.Z 或留空）
- 客户端 `version_check.rs` 启动 5s 后拉 `/api/config`，版本低于最低要求时：
  - 翻 `Arc<AtomicBool>::upload_blocked` 标志 → metrics + conversation 两个 loop 立即跳过
  - Tauri emit `force-update` 事件，带 `{current, min_required}`
  - 网络失败或服务端未配置 → fail-open，不阻塞
- 客户端 React `ForceUpdateOverlay` 组件监听事件 → 全屏半透明遮罩（不可关闭，无关闭按钮），提示用户联系管理员升级

### Changed
- 两个 upload loop (metrics + conversation) 启动时共享同一个 `upload_blocked` flag，由 `version_check` 控制

## [0.5.2] - 2026-04-22

### Added

#### Cursor Prompt 采集
- conversation collection 扩展到 Cursor，读 `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb` (SQLite)
- 幂等键 = `{composer_id}_{sha256(text)[:16]}`（Cursor bubble 无原生 uuid）
- Per-composer 水位线 `CursorMark { last_updated_at, bubble_index }`（SQLite 不是 append-only，byte offset 不适用）
- Model 回填：user bubble `model_name` 为空时，回溯找最近一条有 model 的 bubble
- 系统注入过滤复用 codex 的 `is_system_injection`（`<snake_tag>` / `<ALLCAPS>` / `# AGENTS.md`）
- `lib.rs` flush 传 `&["claude_code", "codex", "cursor"]`
- 新增 `sha2` 依赖

#### Dashboard
- "查看问题"按钮启用条件扩到 `['claude_code', 'codex', 'cursor']`
- Codex + Cursor 场景抽屉跳过 model 过滤（metrics↔conversation 的 model 字段语义不一致，长期应在 scanner 对齐）

### Fixed
- `codex_scanner` 把窄的 `<xxx_context>` 过滤规则推广到 lowercase snake_case tag（含下划线，避免误伤 `<div>` 等 HTML）→ 捕获 `<turn_aborted>` 等
- 新增 `<INSTRUCTIONS>` 类全大写 tag 和 `# AGENTS.md` Markdown 前缀的过滤

## [0.5.1] - 2026-04-22

### Added

#### Codex Prompt 采集
- conversation collection 扩展到 Codex（`~/.codex/sessions/{Y}/{M}/{D}/rollout-*.jsonl`）
- 幂等键 = `{session_id}_{line_start_offset}`（Codex 消息行没有原生 uuid，用合成 key）
- 系统消息过滤：`role ∈ {system, developer}` 或首行匹配 `<xxx_context>` 模式（catches `<environment_context>` / `<task_context>` 等）
- Model 回填：扫描 user 消息后的 `turn_context.payload.model` 行（Codex 不在 assistant 消息上挂 model）
- `uploader::flush` 签名改为 `flush(server_url, tools: &[&str])`，共享 state/HTTP client，顺序上报多工具
- lib.rs 现在 `flush(&server, &["claude_code", "codex"])`

#### Dashboard
- "查看问题"按钮启用条件扩到 `['claude_code', 'codex']`
- 抽屉 title 动态显示工具名（Claude Code / Codex）

## [0.5.0] - 2026-04-22

### Added

#### Claude Code 用户 Prompt 采集
- 后台扫描 `~/.claude/projects/**/*.jsonl`，按文件字节 offset 增量抽取 user prompt，上传至 `ai-usage-server` 的 `/api/conversations` 端点
- 独立于现有用量指标上报循环：启动 60s 后首发，之后每 5 分钟
- 10MB 分批、断点续传；状态文件 `{data_dir}/session-viewer/conversation-state.json`
- 过滤 6 类 CLI 注入消息（`<system-reminder>` / `<command-name>` 等）与 `tool_result`
- 启发式 `first` / `followup` / `retry` 标签；模型从紧跟的 assistant 消息回填
- 服务端纯文件存储（NDJSON），保留 90 天后自动清理，无需数据库

#### Dashboard 查看问题入口
- "项目用量明细"新增"操作"列；`claude_code` 行显示"查看问题"按钮，其他工具占位
- 右侧抽屉展示对应 (date × project × model) 的 prompt 明细
- 支持文本搜索与"仅首问"过滤；长 prompt 折叠，点击展开；ESC 关闭

---

## [0.2.0] - 2026-02-07

### Fixed

#### Resume Session — Terminal Lifetime
- **Critical**: Resumed terminals no longer get killed when the app exits
  - **Windows**: Replaced direct `cmd` spawn with `cmd /c start /d` — the `start` command launches a fully independent process owned by Windows shell, not by our app. The intermediate `cmd /c` exits immediately, breaking the parent-child link. `CREATE_NO_WINDOW` hides the brief intermediate cmd flash.
  - **Linux**: Added `process_group(0)` (calls `setsid`) to create an independent process session that survives parent exit.
  - **macOS**: Already independent (Terminal.app owns the process via AppleScript).

#### Linux Build
- Fixed `AsRef<OsStr>` type inference ambiguity caused by `glib` crate on Linux — removed unnecessary `.as_ref()` call in `Command::args`.
- Fixed `format!` temporary `String` lifetime issue — pre-bind formatted strings with `let` before referencing in array.

---

## [0.1.0] - 2026-02-07

First release of Claude Memory Viewer.

### Added

#### Project Browser
- Auto-scan `~/.claude/projects/` directory to discover all Claude Code projects
- Display project path, session count, and last active time
- Sort projects by most recently active

#### Session List
- Read Claude Code's `sessions-index.json` for instant loading
- Show session summary, first prompt preview, message count, Git branch, created/modified timestamps
- One-click Resume button to open `claude --resume {sessionId}` in system terminal

#### Message Detail
- Full conversation rendering with paginated loading (infinite scroll)
- **User messages** — plain text and tool result display
- **Assistant messages** — Markdown rendering with GFM support (tables, task lists, strikethrough)
- **Code blocks** — Syntax highlighting via Prism (oneDark theme), supporting 100+ languages
- **Thinking blocks** — Collapsible display of Claude's reasoning process
- **Tool calls** — Collapsible display of tool name, input parameters, and results
- Large content truncation (2000 chars) with expand option

#### Global Search
- Cross-project, cross-session full-text search
- Parallel scanning powered by Rayon (Rust)
- Case-insensitive matching with keyword highlighting
- Click results to navigate directly to the matching message

#### Token Statistics
- Read `stats-cache.json` for usage data
- Summary cards: total messages, sessions, tool calls, tokens
- Daily activity bar chart (messages + tool calls)
- Token usage trend area chart (input / output over time)
- Model usage distribution with progress bars

#### Resume Session (Cross-platform)
- **Windows** — Opens new CMD window via `cmd /c start`
- **macOS** — Uses AppleScript to open Terminal.app
- **Linux** — Auto-detects gnome-terminal / konsole / xfce4-terminal / xterm

#### Infrastructure
- Tauri v2 desktop app (Rust backend + React frontend)
- React 19 + TypeScript + Vite 6 + Tailwind CSS
- Zustand state management
- GitHub Actions CI (cargo check + clippy + tsc)
- GitHub Actions Release workflow for multi-platform builds (Windows / macOS Intel / macOS ARM / Linux)
- MIT License

### Technical Details

- **JSONL Parser**: Stream-based parsing with `BufReader` + line-level pre-filtering, skips `progress` and `file-history-snapshot` records for performance
- **Session Index**: Leverages Claude Code's built-in `sessions-index.json` for millisecond-level session list loading
- **Search**: Rayon parallel brute-force search across all JSONL files
- **Path Handling**: Cross-platform Claude home detection (`%USERPROFILE%\.claude` on Windows, `~/.claude` on Unix)

[0.2.0]: https://github.com/zuoliangyu/claude-memory-viewer/releases/tag/v0.2.0
[0.1.0]: https://github.com/zuoliangyu/claude-memory-viewer/releases/tag/v0.1.0
