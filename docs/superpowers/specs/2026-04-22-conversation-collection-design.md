# 用户咨询问题采集系统 — 设计文档

> 日期: 2026-04-22
> 涉及仓库: `session-viewer` (客户端) + `ai-usage-server` (服务端)
> 状态: 设计完成，待评审

## 1. 背景与目标

现有系统 (`session-viewer` → `ai-usage-server`) 已在定期收集各 AI 编码助手的用量指标 (token、session_count、message_count 等)，并通过 SQLite 聚合展示在 Dashboard。

本次需求：**扩展采集范围，把"用户实际提问的 prompt 原文"也统一收集到服务端**，以便后续分析高频问题、构建 FAQ / 知识库。

- 分析目标：FAQ 聚合（识别常见提问、发现共性痛点）
- 覆盖范围 (MVP)：客户端仅 Claude Code；服务端设计为多工具兼容
- 隐私模式：沿用 `git email` 身份，默认开启，明文上报（内部团队）
- 保留期：服务端 3 个月 TTL
- 独立于现有 `/api/report` 指标上报流程，走独立端点、独立调度

## 2. 核心决策记录

| # | 决策项 | 结论 | 备注 |
|---|---|---|---|
| 1 | 分析目标 | FAQ 聚合 | 只需 user prompt 文本 + 维度信息 |
| 2 | 身份与同意 | `git email` 明文，隐式开启 | 与现有用量上报一致 |
| 3 | 工具范围 | 客户端 MVP 只做 Claude Code；服务端架构兼容后续接入 Codex/OpenCode/Copilot/Cursor | |
| 4 | 采集粒度 | 所有 user prompt 全采，客户端打 `first/followup/retry` 标签 | |
| 5 | 单条体积 | 不截断 | 服务端 3 个月 TTL + 扩容磁盘兜底 |
| 6 | 增量策略 | 按文件字节 offset 作水位线；10MB/批，断点续传；首次全量，后续增量 | 去重键: Claude `message_uuid` |
| 7 | 服务端存储 | 纯文件 NDJSON (date-first 目录)，**不进数据库** | 规避 SQLite 性能风险 |
| 8 | UI 入口 | Dashboard "项目用量明细"表新增"操作"列 → 抽屉展示该 cell 对应的 prompt 明细 | |

## 3. 总体架构

```
客户端 session-viewer (MVP: Claude only)
  └─ 新增模块 src-tauri/src/conversation/
       ├─ scanner.rs   扫 jsonl + 过滤 + 打 role_tag + 回填 model
       ├─ uploader.rs  分批 + 断点续传 + 幂等
       ├─ state.rs     持久化 file_offsets 状态
       └─ mod.rs       模块导出
  └─ lib.rs 新增独立 spawn 任务 `conversation_loop`（与 metrics auto-report 并行）
  └─ Cargo.toml 无新增依赖

服务端 ai-usage-server
  └─ src/conversations.ts  新路由
       ├─ POST /api/conversations         接收上报（无鉴权，与 /api/report 一致）
       └─ GET  /api/conversations/detail  查询（JWT 鉴权，供 Dashboard 调用）
  └─ src/cleanup.ts        3 个月 TTL 清理调度
  └─ src/index.ts          注册路由 + 启动 cleanup
  └─ 存储路径: /app/data/conversations/{tool}/{YYYY-MM-DD}/{sanitize(email)}.jsonl
  └─ Docker: 复用现有 /app/data volume；运维侧扩磁盘

Dashboard public/index.html
  └─ "项目用量明细"表新增"操作"列
  └─ 复用 .modal-overlay；新增 .drawer 样式从右侧滑入
  └─ renderConversationDrawer() 展示 prompt 明细，支持搜索 / 仅首问过滤
```

关键不变量：
- `message_uuid` 在客户端侧做幂等键，服务端不做去重（纯追加）
- 服务端无状态，目录路径即路由；文件按日期分区，方便 TTL 清理
- Metrics 上报（`/api/report` → SQLite）和 Conversation 上报（`/api/conversations` → 文件）完全解耦

## 4. 数据结构

### 4.1 客户端上报 Payload

`POST /api/conversations` 请求体：
```json
{
  "user_email": "bin@example.com",
  "user_name": "bin",
  "machine_id": "bin-mbp",
  "client_version": "0.5.0",
  "tool": "claude_code",
  "reported_at": "2026-04-22T10:45:00Z",
  "messages": [ <ConversationMessage>, ... ]
}
```

### 4.2 ConversationMessage

```json
{
  "uuid": "msg-uuid-from-jsonl",      // 来自 jsonl 行顶层 uuid 字段
  "session_id": "session-uuid",       // 来自 jsonl 行顶层 sessionId 字段
  "parent_uuid": "prev-msg-uuid",     // 可选，来自 jsonl 行顶层 parentUuid 字段
  "timestamp": "2026-04-22T09:24:08.302Z",
  "project": "session-viewer",         // cwd basename
  "cwd": "/Users/bin/.../session-viewer",
  "git_branch": "main",                // jsonl 有就带
  "model": "claude-opus-4-6",          // 从紧跟的 assistant 消息回填，取不到则 null
  "role_tag": "first",                 // first | followup | retry
  "text": "<prompt 原文，不截断>"
}
```

**`model` 回填规则**：
- 扫描 jsonl 时，user 消息本身无 model 字段。按行顺序查找紧随其后的第一条 `type=assistant` 消息，取其 `message.model`，反写到 user 消息上。
- 如 user 是 session 最后一条（无后续 assistant 回复），或后续 assistant 的 model 为 `<synthetic>`/`unknown`，则 `model = null`。

### 4.3 `role_tag` 规则（客户端判定）

- `first`: 该 session 内第一条未被过滤的 user prompt。
  **增量扫描下的判定**：
  - 若本次扫描的 `start_offset == 0`（首次见此文件）→ 本文件内第一条未过滤的 user prompt 为 `first`
  - 若 `start_offset > 0`（续扫）→ 默认**不再产生 `first`**，即使本次看到的第一条也按 `followup` / `retry` 处理
  - 含义：`first` 标签只在从文件零字节开始扫时才能判定；后续增量不追溯
  - 边界：若文件首次扫描时极短、尚无 user 消息，会错过 `first` 标记。MVP 接受此轻微误差
- `retry`: 文本长度 ≤ 30 字符 且 匹配以下正则之一（大小写不敏感）：
  - `^(再试|重试|不对|继续|换一个)`
  - `^(retry|try again|again|no|continue|go on)\b`
- `followup`: 其他所有

### 4.4 过滤规则（客户端跳过不上报）

以下 user 消息一律不进入上报队列：
- 以 6 类 CLI 注入前缀开头的 content：
  `<local-command-caveat>` / `<command-name>` / `<local-command-stdout>` / `<local-command-stderr>` / `<system-reminder>` / `<system-status>`
- `message.content` 为空串或纯空白
- tool_result 类型的消息（非真正用户输入）
- `message.content` 为数组形式且只含 `tool_result` / `tool_use` 等非文本类型

文本提取：`message.content` 若为字符串直接用；若为数组，取所有 `type=text` 元素的 `text` 字段 `\n\n` 拼接。

### 4.5 服务端落盘格式

路径：`/app/data/conversations/{tool}/{YYYY-MM-DD}/{sanitize(email)}.jsonl`

每行是 `ConversationMessage` 加上服务端注入的元数据：
```json
{ ...ConversationMessage,
  "user_email": "bin@example.com",
  "user_name": "bin",
  "machine_id": "bin-mbp",
  "client_version": "0.5.0",
  "received_at": "2026-04-22T10:45:01Z" }
```

**分桶日期取自消息的 `timestamp[0..10]`**（不是 `reported_at`）——跨午夜的 batch 会按消息时间自动落到各自日期目录。

`sanitize(email)` 规则：
- 允许字符：`a-zA-Z0-9._@+-`
- 其他一律替换为 `_`
- 例：`bin@example.com` → `bin@example.com`（本就安全，原样保留）

## 5. 客户端详细流程

### 5.1 新增文件

```
src-tauri/src/conversation/
  ├── mod.rs           // 模块导出 + 常量
  ├── scanner.rs       // 扫 jsonl，产出 ConversationMessage 流
  ├── uploader.rs      // 批次管理 + HTTP 发送
  └── state.rs         // file_offsets 持久化
```

### 5.2 本地状态文件

路径：`{data_dir}/session-viewer/conversation-state.json`
（`data_dir` 由 `dirs::data_dir()` 决定，与现有 `report-high-water.json` 同目录）

```json
{
  "file_offsets": {
    "/Users/bin/.claude/projects/-Users-bin-foo/abc.jsonl": 45231,
    "/Users/bin/.claude/projects/-Users-bin-foo/def.jsonl": 8190
  },
  "last_scan_at": "2026-04-22T10:40:00Z"
}
```

### 5.3 扫描算法 (`scanner.rs`)

输入：`file_offsets` HashMap<PathBuf, u64>
输出：`Vec<(PathBuf, u64, ConversationMessage)>` — 每项记消息所在文件、本消息"行末字节位置"、消息本体

```rust
pub fn scan_incremental(offsets: &HashMap<PathBuf, u64>) -> Vec<PendingMessage> {
    let projects_dir = get_projects_dir()?;
    let mut results = Vec::new();

    for project_dir in read_dir(projects_dir) {
        for jsonl_path in read_dir(project_dir).filter(is_jsonl) {
            let start_offset = offsets.get(&jsonl_path).copied().unwrap_or(0);
            let file_size = metadata(&jsonl_path).len();

            // 文件被截断/轮转 → 重置
            let start_offset = if start_offset > file_size { 0 } else { start_offset };

            let mut file = File::open(&jsonl_path)?;
            file.seek(Start(start_offset))?;
            let mut reader = BufReader::new(file);
            let mut cursor = start_offset;

            // 先读完整个增量，建立 index，再回填 model
            let mut lines: Vec<(u64, serde_json::Value)> = Vec::new();
            loop {
                let mut buf = String::new();
                let n = reader.read_line(&mut buf)?;
                if n == 0 { break; }
                cursor += n as u64;
                if let Ok(v) = serde_json::from_str(buf.trim()) {
                    lines.push((cursor, v));
                }
            }

            // 顺序扫：对每个符合条件的 user 消息，查找紧随的 assistant 取 model
            let mut session_first_seen: HashSet<String> = HashSet::new();
            for (i, (line_end, v)) in lines.iter().enumerate() {
                if v["type"] != "user" { continue; }
                let Some(msg) = extract_user_message(v) else { continue };  // 含过滤规则
                let model = lookup_following_model(&lines[i+1..]);  // 查下一条 assistant
                let session_id = v["sessionId"].as_str().unwrap_or("");
                let role_tag = classify_role(&msg.text, session_id, &mut session_first_seen);

                results.push(PendingMessage {
                    file: jsonl_path.clone(),
                    line_end: *line_end,
                    message: ConversationMessage { /* ... */ model, role_tag, ... },
                });
            }
        }
    }
    results
}
```

**注意**：由于首次运行时所有 `offsets` 为空，此算法会扫完历史全部消息。消息数量可能到十万级；需流式处理避免 OOM（实际实现可用 iterator 而非 Vec）。

### 5.4 上传与水位推进 (`uploader.rs`)

```rust
pub async fn flush() {
    let mut state = load_state();
    let pending = scanner::scan_incremental(&state.file_offsets);
    if pending.is_empty() { return; }

    // 切成 <=10MB batch
    for batch in split_into_batches(pending, 10 * 1024 * 1024) {
        match send_batch(&batch).await {
            Ok(_) => {
                // 对每个文件，更新 offset 到本批该文件最大 line_end
                for (file, max_end) in max_offsets_by_file(&batch) {
                    state.file_offsets.insert(file, max_end);
                }
                state.last_scan_at = Utc::now().to_rfc3339();
                save_state(&state);  // 每批成功后立即落盘 → 断点续传
            }
            Err(e) if is_client_error(&e) => {
                // 4xx：标记日志，推进 offset 避免死循环
                log_dead_letter(&batch, &e);
                for (file, max_end) in max_offsets_by_file(&batch) {
                    state.file_offsets.insert(file, max_end);
                }
                save_state(&state);
            }
            Err(e) => {
                // 5xx / 网络错：不推进 offset，下轮重试
                eprintln!("[Conversation] batch failed: {e}");
                break;
            }
        }
    }
}
```

**切 batch 规则**：
- 按 `serde_json::to_vec(&msg).len()` 累加字节，超过 10MB 开新批
- 单条消息 > 10MB 的（极罕见）单独成批，payload 可能略超 10MB，通过调大 HTTP body 限制容忍

**HTTP 参数**：
- URL: `{server}/api/conversations`
- Method: POST
- Timeout: 60s（单批 ≤ 10MB）
- HTTP client：复用 `report.rs` 的 `reqwest::Client::builder().no_proxy().build()` 逻辑（绕过系统代理）

### 5.5 调度循环 (`lib.rs` 集成)

```rust
// 已有: auto-report metrics loop
tauri::async_runtime::spawn(async { /* ... */ });

// 新增: conversation loop
tauri::async_runtime::spawn(async {
    tokio::time::sleep(Duration::from_secs(30)).await;  // 与 metrics 错开
    loop {
        eprintln!("[Conversation] scanning & uploading");
        if let Err(e) = conversation::uploader::flush().await {
            eprintln!("[Conversation] error: {e}");
        }
        tokio::time::sleep(Duration::from_secs(300)).await;  // 5min
    }
});
```

### 5.6 错误处理矩阵

| 场景 | 行为 |
|------|------|
| 网络断开 / 5xx | 当前批不推进 offset，下轮重试；日志 warn |
| 4xx（payload 格式错误） | 写 dead-letter 日志 `{data_dir}/session-viewer/conversation-errors.log`，推进 offset 避免死循环 |
| state 文件损坏 | 重置为空 → 全量重扫；服务端会重复接收，分析侧 `sort -u uuid` 去重 |
| jsonl 文件被截断（offset > file_size） | offset 重置为 0，重扫该文件 |
| jsonl 文件被删除 | 下次扫描时自动从 state 清除 |
| 首次启动 | file_offsets 为空，扫全部 jsonl，可能产生 GB 级数据，分批稳步上传直至追平 |

## 6. 服务端详细流程

### 6.1 POST /api/conversations

```typescript
router.post("/api/conversations", async (req, res) => {
  try {
    const { user_email, user_name, machine_id, client_version, tool, reported_at, messages } = req.body;
    if (!user_email || !tool || !Array.isArray(messages)) {
      return res.status(400).json({ error: "Missing required fields" });
    }

    // 按消息 timestamp[0..10] 分桶
    const byDate = new Map<string, any[]>();
    for (const m of messages) {
      const date = (m.timestamp || "").slice(0, 10);
      if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) continue;
      const line = {
        ...m,
        user_email, user_name, machine_id, client_version,
        received_at: new Date().toISOString(),
      };
      if (!byDate.has(date)) byDate.set(date, []);
      byDate.get(date)!.push(line);
    }

    for (const [date, lines] of byDate) {
      const dir = path.join(DATA_ROOT, tool, date);
      await fs.promises.mkdir(dir, { recursive: true });
      const file = path.join(dir, `${sanitizeEmail(user_email)}.jsonl`);
      await appendLinesAtomic(file, lines);
    }

    res.json({ ok: true, received: messages.length });
  } catch (e: any) {
    console.error("Conversation upload error:", e);
    res.status(500).json({ error: e.message });
  }
});
```

`appendLinesAtomic` 实现要点：
- 整批消息先序列化成一个 Buffer（`lines.map(l => JSON.stringify(l)).join('\n') + '\n'`）
- `fs.open(file, 'a')` → O_APPEND 模式
- Unix：`proper-lockfile` 或 `fs-ext.flock(fd, 'ex')` 防并发（若部署为单 node 进程可省，MVP 省）
- `fs.write(fd, buffer)` 单次系统调用（写入 < 16MB 时基本原子）
- `fs.fsync(fd)` 落盘
- `fs.close(fd)`

`express.json({ limit: '15mb' })`（当前可能较小，需核查并调整）

### 6.2 GET /api/conversations/detail（Dashboard 查询）

```typescript
router.get("/api/conversations/detail", requireAuth, async (req, res) => {
  try {
    const { tool, date, email, project, model } = req.query as Record<string, string>;
    if (!tool || !date || !email || !project) {
      return res.status(400).json({ error: "Missing params" });
    }
    const file = path.join(DATA_ROOT, tool, date, `${sanitizeEmail(email)}.jsonl`);
    if (!fs.existsSync(file)) {
      return res.json({ total: 0, messages: [] });
    }
    const content = await fs.promises.readFile(file, "utf-8");
    const messages = content.split("\n").filter(Boolean).map(l => {
      try { return JSON.parse(l); } catch { return null; }
    }).filter(m => m
      && m.project === project
      && (!model || m.model === model || (model === "unknown" && !m.model))
    );
    messages.sort((a, b) => (a.timestamp || "").localeCompare(b.timestamp || ""));
    res.json({ total: messages.length, messages });
  } catch (e: any) {
    res.status(500).json({ error: e.message });
  }
});
```

**说明**：
- MVP 阶段整读文件，不分页（单天单用户通常 < 5MB，内存读取可接受）
- 若后续单文件数据量增大，加 `?limit=500&cursor=<uuid>` 分页
- 返回字段：全部消息字段 + 服务端元数据（`received_at` 等）

### 6.3 3 个月 TTL 清理 (`src/cleanup.ts`)

```typescript
export function startCleanupScheduler(dataRoot: string) {
  // 启动时先跑一次，之后每 6h 触发
  setTimeout(runCleanup, 30_000);
  setInterval(runCleanup, 6 * 60 * 60 * 1000);

  function runCleanup() {
    const cutoff = formatDate(Date.now() - 90 * 86400_000);  // YYYY-MM-DD
    const convoRoot = path.join(dataRoot, "conversations");
    if (!fs.existsSync(convoRoot)) return;

    for (const tool of fs.readdirSync(convoRoot)) {
      const toolDir = path.join(convoRoot, tool);
      for (const dateDir of fs.readdirSync(toolDir)) {
        if (!/^\d{4}-\d{2}-\d{2}$/.test(dateDir)) continue;
        if (dateDir < cutoff) {
          const full = path.join(toolDir, dateDir);
          const size = getDirSize(full);
          fs.rmSync(full, { recursive: true, force: true });
          console.log(`[Cleanup] removed ${full} (${(size/1024/1024).toFixed(1)} MB)`);
        }
      }
    }
  }
}
```

目录名本身就是日期，直接字符串比较 `dateDir < cutoff` 即可判断过期，不依赖文件 mtime。

### 6.4 Docker / 部署

- `/app/data` volume 已存在（当前挂 SQLite）；`conversations/` 作为子目录共用
- Dockerfile 无需改动
- 运维侧：根据团队规模扩展挂载磁盘（估算 10 人 × 5MB/天 × 90d ≈ 4.5GB，建议预留 ≥ 20GB）
- `src/index.ts` 启动时调用 `startCleanupScheduler(DATA_ROOT)`

## 7. Dashboard UI 改造

### 7.1 文件位置

唯一改动：`public/index.html`

已确认：纯原生 HTML + CSS + inline `<script>`，无框架，无 CDN，无构建步骤。改动遵循此风格，不引入新依赖。

### 7.2 明细表新增"操作"列

L177-188 `<thead>`：
```html
<th>操作</th>
```

L640-655 `renderDetailTable`：
```js
<td>${r.tool === 'claude_code'
  ? `<button class="view-btn" data-date="${r.date}" data-project="${escapeAttr(r.project)}" data-model="${escapeAttr(r.model)}" onclick="openConversationsFromBtn(this)">查看问题</button>`
  : `<span style="color:#ccc" title="暂未接入">—</span>`}</td>
```

`escapeAttr` 与 `openConversationsFromBtn` 避免 project/model 中包含引号导致 HTML 注入或 onclick 参数解析错误：
```js
function escapeAttr(s) {
  return String(s ?? '').replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}
function openConversationsFromBtn(btn) {
  openConversations(btn.dataset.date, btn.dataset.project, btn.dataset.model);
}
```

`view-btn` 样式（加到 `<style>` 块）：
```css
.view-btn { padding: 4px 10px; font-size: 12px; border: 1px solid #6c5ce7; background: #fff; color: #6c5ce7; border-radius: 4px; cursor: pointer; }
.view-btn:hover { background: #6c5ce7; color: #fff; }
```

### 7.3 抽屉组件

CSS：
```css
.drawer-overlay { display: none; position: fixed; inset: 0; background: rgba(0,0,0,0.3); z-index: 900; }
.drawer-overlay.active { display: block; }
.drawer { position: fixed; top: 0; right: 0; height: 100vh; width: min(600px, 100vw); background: #fff; box-shadow: -4px 0 16px rgba(0,0,0,0.1); transform: translateX(100%); transition: transform 0.25s ease; z-index: 901; display: flex; flex-direction: column; }
.drawer.active { transform: translateX(0); }
.drawer-header { padding: 16px 20px; border-bottom: 1px solid #eee; }
.drawer-filters { padding: 12px 20px; border-bottom: 1px solid #f0f0f0; display: flex; gap: 12px; align-items: center; }
.drawer-body { flex: 1; overflow-y: auto; padding: 0 20px; }
.msg-item { padding: 12px 0; border-bottom: 1px solid #f5f5f5; }
.msg-meta { font-size: 12px; color: #888; margin-bottom: 4px; }
.msg-tag { display: inline-block; padding: 1px 6px; border-radius: 3px; font-size: 11px; background: #f0f0ff; color: #6c5ce7; margin-left: 4px; }
.msg-tag.followup { background: #fff4e6; color: #d97706; }
.msg-tag.retry { background: #ffe4e6; color: #dc2626; }
.msg-text { white-space: pre-wrap; word-break: break-word; font-size: 13px; line-height: 1.5; max-height: 8em; overflow: hidden; cursor: pointer; }
.msg-text.expanded { max-height: none; }
```

HTML（加到 body 末尾）：
```html
<div class="drawer-overlay" id="drawerOverlay" onclick="closeDrawer()"></div>
<div class="drawer" id="drawer">
  <div class="drawer-header">
    <div id="drawerTitle"></div>
    <button class="back-btn" onclick="closeDrawer()">关闭</button>
  </div>
  <div class="drawer-filters">
    <input type="text" id="drawerSearch" placeholder="搜索文本…" oninput="filterDrawer()">
    <label><input type="checkbox" id="drawerFirstOnly" onchange="filterDrawer()"> 仅首问</label>
    <span id="drawerCount"></span>
  </div>
  <div class="drawer-body" id="drawerBody"></div>
</div>
```

JS：
```js
let drawerMessages = [];

async function openConversations(date, project, model) {
  const email = currentDetailEmail;
  const url = `/api/conversations/detail?tool=claude_code&date=${date}&email=${encodeURIComponent(email)}&project=${encodeURIComponent(project)}&model=${encodeURIComponent(model)}`;
  const r = await fetch(url);
  const { messages } = await r.json();
  drawerMessages = messages;
  document.getElementById('drawerTitle').innerHTML =
    `<strong>${email}</strong> · ${project} · ${date}<br><span style="font-size:12px;color:#888">${model} · Claude Code</span>`;
  document.getElementById('drawerSearch').value = '';
  document.getElementById('drawerFirstOnly').checked = false;
  renderDrawerBody(messages);
  document.getElementById('drawerOverlay').classList.add('active');
  document.getElementById('drawer').classList.add('active');
}

function closeDrawer() {
  document.getElementById('drawerOverlay').classList.remove('active');
  document.getElementById('drawer').classList.remove('active');
}

function filterDrawer() {
  const kw = document.getElementById('drawerSearch').value.toLowerCase();
  const firstOnly = document.getElementById('drawerFirstOnly').checked;
  const filtered = drawerMessages.filter(m =>
    (!firstOnly || m.role_tag === 'first')
    && (!kw || (m.text || '').toLowerCase().includes(kw))
  );
  renderDrawerBody(filtered);
}

function renderDrawerBody(list) {
  document.getElementById('drawerCount').textContent = `共 ${list.length} 条`;
  document.getElementById('drawerBody').innerHTML = list.map((m, i) => `
    <div class="msg-item">
      <div class="msg-meta">
        ${m.timestamp}
        <span class="msg-tag ${m.role_tag}">${m.role_tag}</span>
      </div>
      <div class="msg-text" onclick="this.classList.toggle('expanded')">${escapeHtml(m.text || '')}</div>
    </div>
  `).join('') || '<div class="empty">无数据</div>';
}

function escapeHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
```

## 8. 测试策略

### 8.1 客户端单元测试 (`src-tauri/src/conversation/`)

- `scanner::classify_role` — 构造各种 text 验证 first/followup/retry 判定
- `scanner::lookup_following_model` — 构造 jsonl 片段，验证 model 回填（含 synthetic 跳过）
- `scanner::extract_user_message` — 验证 6 类系统注入前缀被过滤
- `uploader::split_into_batches` — 边界：空输入、单条 > 10MB、正好 10MB

### 8.2 客户端集成测试

- Mock HTTP server，首次启动跑一轮 flush，断言 state.file_offsets 被正确推进
- 模拟 5xx → offset 不推进；重启后可续传
- 模拟 4xx → dead letter 写入 + offset 推进
- 构造一个已截断的 jsonl（offset > size）→ 验证重置

### 8.3 服务端测试

- POST /api/conversations：正常批次、空 messages、非法 timestamp、跨天 batch
- GET /api/conversations/detail：文件不存在、project 过滤、model 过滤
- Cleanup：构造 100 天前目录，验证被删；60 天前目录，验证保留
- 并发追加：两个请求同时写同一文件，验证无行撕裂

### 8.4 端到端手工冒烟

1. 在客户端 `~/.claude/projects/` 下有真实数据
2. 启动 `npm run tauri dev`，观察 `[Conversation]` 日志
3. 等 30s 触发首轮，检查服务端 `/app/data/conversations/claude_code/YYYY-MM-DD/*.jsonl` 产生
4. Dashboard 打开对应用户详情，"项目用量明细"点"查看问题"，抽屉出现消息
5. 搜索框、仅首问过滤均工作

## 9. 回滚与兼容

- 服务端两个新端点与现有 API 完全隔离；禁用只需注释路由注册，不影响 `/api/report`
- 客户端 `conversation_loop` 独立 spawn；失败不影响 `auto-report` metrics 上报
- 状态文件 `conversation-state.json` 与 `report-high-water.json` 分开；删除前者等同重置该功能
- 数据层面：`conversations/` 目录独立于 `usage.db`，可整体删除无副作用

## 10. 开放事项（不阻塞本次落地）

- **分析工具**：后续用什么工具消费 NDJSON（jq 脚本 / Python notebook / 接入 Elasticsearch？）本次不实现
- **跨天 / 跨用户聚合查询**：MVP 的 `/api/conversations/detail` 仅按单文件查。若后续要做"全团队一周高频问题"，可能需要离线 ETL 或轻量索引
- **其他工具接入**：Codex/OpenCode/Copilot/Cursor 的 user prompt 抽取逻辑后续按需实现，服务端与 Dashboard 已兼容
- **压缩归档**：>24h 文件自动 gzip，需单独实现；MVP 阶段依赖磁盘扩容

## 11. 实施顺序建议（交给 writing-plans）

1. 客户端 `conversation/` 模块（scanner + state + uploader，含单测）
2. 客户端 `lib.rs` 接入独立 spawn
3. 服务端 `conversations.ts` 路由（POST + GET）
4. 服务端 `cleanup.ts` + `index.ts` 接入
5. Dashboard `public/index.html` 增加"操作"列 + 抽屉
6. 端到端冒烟 + 文档收尾
