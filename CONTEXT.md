# AI Session Viewer - 完整上下文记录

> 最后更新: 2026-04-13

## 项目概述

基于 **Tauri 2 + React + TypeScript + Rust** 的桌面应用，用于可视化浏览多种 AI 编码助手的本地会话记录，并自动上报使用统计到远程服务器。

- **仓库位置**: `/Users/bin/Desktop/cache/cache3/session-viewer`
- **参考上游仓库**: https://github.com/zuoliangyu/AI-Session-Viewer
- **当前分支**: `main`
- **版本**: 0.1.0
- **平台**: macOS (Darwin 25.3.0, aarch64)

## 技术栈

| 层 | 技术 |
|---|------|
| 桌面框架 | Tauri v2 (Rust + WebView) |
| 前端 | React 19 + TypeScript + Vite 6 |
| 样式 | Tailwind CSS 3 |
| 状态管理 | Zustand 5 |
| 图表 | Recharts 2 |
| Markdown 渲染 | react-markdown + remark-gfm + react-syntax-highlighter |
| 图标 | Lucide React |
| 日期 | date-fns 4 |
| 文件监听 | notify 7 (Rust) |
| 并行搜索 | Rayon (Rust) |
| 缓存 | LRU (Rust) |
| HTTP 客户端 | reqwest 0.12 (Rust) |

## 支持的 AI 工具 (5种)

| 工具 | Rust 模块 | 数据源位置 |
|------|-----------|-----------|
| Claude Code | `claude/` | `~/.claude/projects/{encoded-path}/*.jsonl` |
| Codex | `codex/` | Codex CLI 会话文件 |
| OpenCode | `opencode/` | OpenCode 会话文件 |
| Copilot CLI | `copilot/` | `~/.copilot/session-state/{id}/events.jsonl` |
| Cursor | `cursor/` | Cursor 会话文件 |

## 项目结构

```
session-viewer/
├── src/                          # 前端 React + TypeScript
│   ├── components/
│   │   ├── bookmark/
│   │   ├── chat/
│   │   ├── layout/
│   │   │   ├── AppLayout.tsx     # 主布局 (Sidebar + Outlet)
│   │   │   ├── Sidebar.tsx       # 侧边栏 (工具切换/搜索/统计/项目列表)
│   │   │   └── ThemePicker.tsx   # 主题选择器
│   │   ├── message/
│   │   │   ├── MessagesPage.tsx  # 消息详情页 (聊天记录)
│   │   │   ├── MessageThread.tsx # 消息线程渲染
│   │   │   ├── UserMessage.tsx
│   │   │   ├── AssistantMessage.tsx
│   │   │   └── utils.ts
│   │   ├── project/
│   │   │   └── ProjectsPage.tsx  # 项目列表页面
│   │   ├── quick-chat/
│   │   ├── search/
│   │   │   └── SearchPage.tsx    # 全局搜索页面
│   │   ├── session/
│   │   │   ├── SessionsPage.tsx  # 会话列表页面
│   │   │   └── OpencodeSessionList.tsx
│   │   └── stats/
│   │       └── StatsPage.tsx     # 统计仪表盘
│   ├── hooks/
│   │   └── useTheme.ts           # 主题 Hook
│   ├── services/
│   │   └── tauriApi.ts           # Tauri IPC 调用层
│   ├── stores/
│   │   └── appStore.ts           # Zustand 状态管理
│   ├── types/
│   │   └── index.ts              # TypeScript 类型定义
│   ├── App.tsx                   # 路由定义
│   ├── main.tsx                  # React 入口
│   └── index.css                 # Tailwind 全局样式
├── src-tauri/                    # Tauri 桌面端 Rust 后端
│   ├── src/
│   │   ├── claude/
│   │   │   ├── commands/
│   │   │   │   ├── mod.rs        # 模块导出
│   │   │   │   ├── messages.rs   # 消息查询
│   │   │   │   ├── projects.rs   # 项目列表 (含 cwd 路径修正)
│   │   │   │   ├── sessions.rs   # 会话列表 (含模型提取)
│   │   │   │   ├── stats.rs      # 统计计算 (动态扫描 JSONL)
│   │   │   │   ├── search.rs     # 全局搜索
│   │   │   │   ├── terminal.rs   # 恢复会话 (打开终端)
│   │   │   │   └── report.rs     # 🆕 使用数据采集
│   │   │   ├── models/
│   │   │   │   ├── message.rs
│   │   │   │   ├── project.rs
│   │   │   │   ├── session.rs
│   │   │   │   └── stats.rs
│   │   │   └── parser/
│   │   │       ├── jsonl.rs      # JSONL 解析器
│   │   │       └── path_encoder.rs
│   │   ├── codex/                # Codex CLI 支持
│   │   ├── opencode/             # OpenCode 支持
│   │   ├── copilot/              # Copilot CLI 支持
│   │   ├── cursor/               # Cursor 支持
│   │   ├── commands.rs           # 🔑 命令分发 (dispatch by tool)
│   │   ├── lib.rs                # Tauri 入口 (插件/命令注册/自动上报)
│   │   ├── main.rs               # main 入口
│   │   ├── report.rs             # 🆕 多工具使用数据上报
│   │   ├── shared_models.rs      # 共享数据结构
│   │   ├── state.rs              # AppState (LRU 缓存)
│   │   └── watcher/              # 文件系统监听
│   ├── Cargo.toml
│   └── tauri.conf.json
├── dist/                         # 前端构建产物
├── index.html                    # HTML 入口 (含主题初始化脚本)
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── tsconfig.json
└── CONTEXT.md                    # 本文件
```

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────┐
│              React 前端 (WebView)             │
│  ┌──────────┬──────────┬──────────┬────────┐ │
│  │ProjectsPage│SessionsPage│MessagesPage│StatsPage│ │
│  └──────────┴──────────┴──────────┴────────┘ │
│         │ Zustand Store (appStore.ts)         │
│         │ tauriApi.ts (invoke IPC)            │
│         ↕                                     │
├─────────────────────────────────────────────┤
│              Tauri IPC Bridge                 │
├─────────────────────────────────────────────┤
│              Rust 后端                        │
│  ┌─────────────────────────────────────────┐ │
│  │  commands.rs (Dispatch by tool param)   │ │
│  │  ┌───────┬───────┬─────────┬─────────┐  │ │
│  │  │claude/│codex/ │opencode/│copilot/ │  │ │
│  │  │cursor/│       │         │         │  │ │
│  │  └───────┴───────┴─────────┴─────────┘  │ │
│  │  report.rs ──→ HTTP POST to server      │ │
│  │  watcher/ ──→ 文件变更通知               │ │
│  │  state.rs ──→ LRU 缓存                  │ │
│  └─────────────────────────────────────────┘ │
│         ↕                                     │
│  本地文件系统 (~/.claude/, ~/.copilot/, etc.)  │
└─────────────────────────────────────────────┘
         │ 每5分钟自动上报
         ↓
┌─────────────────────────────────────────────┐
│     AI Usage Server (<server>:3000)           │
│  Express + TypeScript + better-sqlite3       │
│  POST /api/report → upsert usage_records     │
│  Dashboard (public/) → 统计可视化             │
└─────────────────────────────────────────────┘
```

### 前端路由

| 路由 | 组件 | 功能 |
|------|------|------|
| `/` | — | 重定向到 `/claude/projects` |
| `/:tool/projects` | ProjectsPage | 项目列表 (按最近修改排序) |
| `/:tool/projects/:projectKey` | SessionsPage | 会话列表 (含模型标签) |
| `/:tool/projects/:projectKey/session/:sessionKey` | MessagesPage | 消息详情 (分页加载) |
| `/:tool/search` | SearchPage | 全局搜索 |
| `/:tool/stats` | StatsPage | 统计仪表盘 |

### Tauri 命令 (10个)

| 命令 | 功能 | 缓存 |
|------|------|------|
| `get_projects` | 项目列表 | 无 |
| `get_sessions` | 会话列表 | 无 |
| `get_sessions_grouped` | 分组会话 (OpenCode) | 无 |
| `get_messages` | 消息分页 | 无 |
| `global_search` | 全局搜索 | 无 |
| `get_stats` | 基础统计 | LRU 缓存 |
| `get_token_summary` | Token 汇总 | LRU 缓存 |
| `get_advanced_stats` | 高级统计 (仅 Claude) | LRU 缓存 |
| `report_usage` | 手动触发上报 | 无 |
| `resume_session` | 恢复会话 (打开终端) | 无 |

### 命令分发模式

`commands.rs` 是统一入口，根据 `tool` 参数路由到各工具模块：

```rust
#[tauri::command]
pub fn get_projects(tool: String) -> Result<Value, String> {
    match tool.as_str() {
        "claude"  => crate::claude::commands::projects::get_projects(),
        "codex"   => crate::codex::commands::projects::get_projects(),
        "opencode"=> crate::opencode::commands::projects::get_projects(),
        "copilot" => crate::copilot::commands::projects::get_projects(),
        "cursor"  => crate::cursor::commands::projects::get_projects(),
        _ => Err("Unknown tool"),
    }
}
```

## 数据源

### Claude Code JSONL 格式

数据存储在 `~/.claude/projects/{encoded-path}/` 目录下：
- 每个项目一个子目录，目录名是路径编码（`/` 替换为 `-`）
- 每个会话一个 `.jsonl` 文件，文件名是 session ID (UUID)
- 可能有 `sessions-index.json` 索引文件

每行一个 JSON 对象，关键 type：
- `"type":"file-history-snapshot"` — 跳过
- `"type":"progress"` — 跳过
- `"type":"user"` — 用户消息
- `"type":"assistant"` — 助手消息

用户消息结构：
```json
{
  "type": "user",
  "uuid": "...",
  "timestamp": "2026-03-08T09:24:08.302Z",
  "sessionId": "...",
  "cwd": "/Users/bin/Desktop/cache/cache3/session-viewer",
  "gitBranch": "main",
  "message": {
    "role": "user",
    "content": "用户文本"
  }
}
```

助手消息结构：
```json
{
  "type": "assistant",
  "uuid": "...",
  "timestamp": "...",
  "message": {
    "model": "claude-opus-4-6",
    "role": "assistant",
    "content": [
      {"type": "thinking", "thinking": "..."},
      {"type": "text", "text": "..."},
      {"type": "tool_use", "id": "...", "name": "Read", "input": {}},
      {"type": "tool_result", "tool_use_id": "...", "content": "..."}
    ],
    "usage": {
      "input_tokens": 3,
      "output_tokens": 11,
      "cache_read_input_tokens": 10098,
      "cache_creation_input_tokens": 24351
    }
  }
}
```

特殊模型名：
- `"<synthetic>"` — CLI 内部占位符消息（token 全为 0，应跳过）
- `"unknown"` — 未知模型，应跳过

### 系统注入的用户消息（非真正用户 prompt）

以下前缀开头的 "user" 消息是 CLI 系统注入的：
- `<local-command-caveat>` / `<command-name>` / `<local-command-stdout>`
- `<local-command-stderr>` / `<system-reminder>` / `<system-status>`

## 使用数据上报 (Report)

### 客户端 (session-viewer)

- **定时任务**: 启动后 30 秒首次上报，之后每 5 分钟
- **上报地址**: `http://<server>:3000/api/report`
- **采集范围**: 所有 5 种工具的使用记录
- **数据结构**: `{ user_email, user_name, machine_id, tool, records[], reported_at }`
- **关键文件**: `src-tauri/src/report.rs`, `src-tauri/src/claude/commands/report.rs`

### 服务端 (ai-usage-server)

- **项目位置**: `/Users/bin/IdeaProjects/ai-usage-server`
- **部署地址**: `<server>:3000`
- **SSH**: 参考内部文档
- **技术栈**: Express + TypeScript + better-sqlite3, Docker 部署
- **数据库路径**: `/app/data/usage.db` (SQLite, 挂载卷持久化)

#### 数据库 Schema

```sql
-- 用户表
CREATE TABLE users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  email TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL DEFAULT '',
  machine_id TEXT NOT NULL DEFAULT '',
  first_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
  last_seen_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 使用记录表
CREATE TABLE usage_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL REFERENCES users(id),
  tool TEXT NOT NULL DEFAULT 'claude_code',
  date TEXT NOT NULL,
  project TEXT NOT NULL,
  model TEXT NOT NULL,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  cache_read_tokens INTEGER NOT NULL DEFAULT 0,
  cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
  session_count INTEGER NOT NULL DEFAULT 0,
  message_count INTEGER NOT NULL DEFAULT 0,
  reported_at TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
-- 唯一索引: (user_id, tool, date, project, model) → 支持 upsert 去重

-- Dashboard 登录账号
CREATE TABLE accounts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,       -- bcrypt
  role TEXT NOT NULL DEFAULT 'admin',
  must_change_pwd INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
-- 默认账号: admin (首次登录需改密)
```

#### API 端点

| 方法 | 路径 | 说明 | 认证 |
|------|------|------|------|
| POST | `/api/report` | 接收客户端上报数据 | **无需认证** |
| GET | `/api/users` | 用户列表 + 汇总统计 | JWT Cookie |
| GET | `/api/stats` | 聚合统计 (支持 group_by: user/project/model/date/tool) | JWT Cookie |
| GET | `/api/stats/detail` | 用户详细记录 | JWT Cookie |
| POST | `/api/auth/login` | 登录 | — |
| GET | `/api/auth/me` | 当前用户信息 | JWT Cookie |
| POST | `/api/auth/change-password` | 修改密码 | JWT Cookie |
| POST | `/api/auth/logout` | 登出 | — |
| GET | `/health` | 健康检查 | — |

#### Docker 部署

```dockerfile
FROM node:20-slim  # 两阶段构建
WORKDIR /app
VOLUME /app/data   # SQLite 持久化
ENV PORT=3000
EXPOSE 3000
CMD ["node", "dist/index.js"]
```

## 关键数据流

### 统计数据加载
```
StatsPage useEffect → loadStats()
  → appStore.loadStats()
    → Promise.all([
        tauriApi.getStats("claude"),         → commands.rs → claude::stats
        tauriApi.getTokenSummary("claude"),   → commands.rs → claude::stats
        tauriApi.getAdvancedStats("claude"),  → commands.rs → claude::stats (仅 Claude)
      ])
    → set({ stats, tokenSummary, advancedStats })
```

### 使用数据上报
```
lib.rs setup() → spawn async task
  → sleep(30s) → loop {
      report::send_all_reports(DEFAULT_REPORT_SERVER)
        → collect records from claude/codex/opencode/copilot
        → POST /api/report { user_email, tool, records[] }
        → server: upsertUser() → insertRecords() (upsert)
      sleep(300s)
    }
```

### 会话列表加载
```
SessionsPage → selectProject(projectKey)
  → tauriApi.getSessions("claude", encodedName)
    → commands.rs → claude::sessions::get_sessions()
      → 读 sessions-index.json + 扫描 .jsonl 文件
      → 补充 models 数据
      → 返回 Vec<SessionIndexEntry>
```

## 已完成的修改历史

### Bug Fixes

1. **统计数据全零** — 当 `~/.claude/stats-cache.json` 不存在时，动态扫描 JSONL 计算统计
2. **模型统计不准** — 跳过 `<synthetic>` 和 `unknown`，模型名只去除 `claude-` 前缀
3. **项目名称不准** — 从 JSONL 的 `cwd` 字段读取真实路径，避免编码名中连字符的歧义
4. **会话标题显示系统消息** — 跳过 6 类系统前缀，找到真正的首条用户 prompt

### Features

1. **高级统计面板** — 项目 Token 排行、工具调用频率排行、会话效率分析
2. **会话模型标签** — 会话列表显示使用的模型名
3. **月度统计筛选** — MonthFilter 组件，按月过滤图表数据
4. **Cursor 支持** — 新增 Cursor 工具支持
5. **使用数据上报** — 自动上报到远程统计服务器 (进行中)

## 构建与开发

```bash
# 开发模式
npm run tauri dev

# 生产打包
npm run tauri build

# 产物位置
# App: src-tauri/target/release/bundle/macos/Session Viewer.app
# DMG: src-tauri/target/release/bundle/dmg/Session Viewer_0.1.0_aarch64.dmg
```

## 已知问题

### OpenCode 模块编译警告 (非本项目引入)

```
warning: unused import: `std::collections::HashMap` → opencode/commands/projects.rs
warning: unused import: `ProjectMetadata` → opencode/commands/projects.rs
warning: unused import: `scan_all_session_files` → opencode/commands/search.rs
warning: fields `id` and `message_id` are never read → opencode/commands/messages.rs
```

### 当前未提交变更 (进行中的工作)

- 新增 `report.rs` — 多工具使用数据上报模块
- 新增 `claude/commands/report.rs` — Claude 使用记录采集
- 修改 `lib.rs` — 引入 report 模块并启动自动上报定时任务
- 修改 `commands.rs` — 注册 `report_usage` 命令
- 修改 `Cargo.toml` — 添加 `reqwest` 和 `hostname` 依赖
- 修改 `StatsPage.tsx` — 前端统计页面更新
- 修改 `tauriApi.ts` — 新增 `reportUsage` API 调用
