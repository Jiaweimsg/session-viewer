# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

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
