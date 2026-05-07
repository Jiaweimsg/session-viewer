# OpenCode 数据读取迁移：JSON 文件 → SQLite

**日期**: 2026-05-07  
**状态**: 已批准，待实现

---

## 背景

opencode 工具已将存储格式从 JSON 目录树升级为 SQLite 数据库（`~/.local/share/opencode/opencode.db`）。原有代码仍在扫描不存在的 `storage/project/`、`storage/session/`、`storage/message/` 目录，导致所有 opencode 数据无法读取，stats 页面 token 数据全为 0。

---

## 目标

- 让 project / session / message 数据正常显示
- stats 页面显示真实 token 用量（数据已在 DB 中）
- 改动仅限 `src-tauri/src/opencode/` 目录

---

## 架构

### 文件变更

**删除**
- `parser/session_scanner.rs` — 目录扫描逻辑
- `parser/json_parser.rs` — JSON 文件解析

**新增**
- `parser/db_reader.rs` — 所有 SQLite 读取逻辑

**更新**
- `models/project.rs` — 移除 `ProjectMetadata`（JSON 专属），保留 `ProjectIndexEntry`
- `models/session.rs` — 清理 JSON 专属死代码字段（`system`、`tools`），保留 `SessionIndexEntry` / `SessionGroup`
- `models/message.rs` — 对齐 DB schema，新增 `tokens` 字段用于 stats
- `commands/projects.rs` — 改调 `db_reader`
- `commands/sessions.rs` — 改调 `db_reader`，时间戳从 DB 列读取
- `commands/messages.rs` — 从 `part` 表读 `type=text` 内容
- `commands/stats.rs` — 聚合 `message.data.tokens` 字段
- `commands/search.rs` — 查 `part` 表 text 做全文搜索

**不动**: `mod.rs`、`commands/terminal.rs`、前端代码

---

## db_reader.rs 接口

```rust
pub fn get_db_path() -> Option<PathBuf>
pub fn open_db() -> Result<Connection, String>   // READ_ONLY | NO_MUTEX

pub fn query_projects(conn: &Connection) -> Vec<ProjectIndexEntry>
pub fn query_sessions(conn: &Connection, project_id: &str) -> Vec<SessionRow>
pub fn query_messages(conn: &Connection, session_id: &str) -> Vec<MessageRow>
pub fn query_parts(conn: &Connection, session_id: &str) -> Vec<PartRow>
```

---

## 数据映射

| DB 列 | 结构体字段 | 说明 |
|---|---|---|
| `project.id` | `ProjectIndexEntry.id` | |
| `project.worktree` | `ProjectIndexEntry.worktree` | 取最后一段作 `short_name` |
| `project.time_updated` | `ProjectIndexEntry.last_modified` | ms → RFC3339 |
| `session.id` | `SessionIndexEntry.session_id` | |
| `session.project_id` | `SessionIndexEntry.project_id` | |
| `session.title` | `SessionIndexEntry.title` | |
| `session.path` | `SessionIndexEntry.directory` | NULL 时 fallback `""` |
| `session.parent_id` | `SessionIndexEntry.parent_id` | 用于父子分组 |
| `session.time_created` | `SessionIndexEntry.created` | ms → RFC3339 |
| `session.time_updated` | `SessionIndexEntry.modified` | ms → RFC3339 |
| `message.data.role` | `MessageRow.role` | `user` / `assistant` |
| `message.data.tokens` | stats 聚合 | `input/output/cache.write/cache.read` |
| `part.data` where `type=text` | `DisplayContentBlock::Text` | 消息正文 |
| `part.data` where `type!=text` | 跳过 | `step-start`/`reasoning`/`patch` 等 |

### first_prompt 逻辑

该 session 最早一条 `role=user` message → 最早一条 `type=text` part → 截取前 100 字符。

### Stats token 聚合

遍历所有 `role=assistant` message，从 `data.tokens` 提取：
- `input` → `total_input_tokens`
- `output` → `total_output_tokens`
- `cache.write` + `cache.read` → cache tokens
- 按 `data.modelID` 分组，按 `time_created` 日期分桶

---

## 错误处理

| 场景 | 处理方式 |
|---|---|
| DB 文件不存在 | `get_db_path()` 返回 `None`，commands 返回空列表 |
| DB 打开失败 | 返回空列表，不 panic |
| JSON 反序列化失败 | `filter_map` 跳过该行 |
| DB 写锁冲突 | `READ_ONLY \| NO_MUTEX`，WAL 模式下读写并发安全 |
| `session.path` 为 NULL | `directory` fallback `""`，`short_name` fallback `"unknown"` |
| part 表无 text 类型 | 消息显示为空内容，不报错 |

---

## 依赖

`rusqlite = { version = "0.31", features = ["bundled"] }` 已在 `Cargo.toml` 中，无需新增依赖。
