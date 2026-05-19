# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.5.25] - 2026-05-19

### Fixed

#### Cursor 使用统计页 UI 漏展 cache_read / cache_write
- 现象：0.5.23 接入 cursor.com 官方接口后,后端 `CursorStats` 已带回完整 `totalCacheReadTokens` / `totalCacheWriteTokens` / `totalTokens`(含 cache 四项之和),但前端 `CursorStatsView` 从未读取这些字段,Token 卡片仅显示 input/output、堆叠图也只画 input+output;在 Agent 长会话场景下用户看到的数字仅占真实 token 的 15-20%(cache_read 通常占 70-85%),对照 cursor.com Dashboard 必然觉得"少了一大半"
- 前端补齐:`CursorDailyTokenEntry` TS 类型增加 `cacheReadTokens` / `cacheWriteTokens` / `cost` 字段(后端早已传入,TS 类型未声明导致编译器看不见);`CursorStatsView` 总览卡片从 7 张改为 10 张(5 列两行),新增 **总 Token (含缓存)** / **Cache Read** / **Cache Write** / **Cursor 计费 ($)** 四张;每日 Token 堆叠柱图从 2 段(input + output)扩为 4 段(cacheRead + cacheWrite + input + output),Tooltip 支持四种中英文 label
- 删除"估算总请求(含Tab)"卡片(×1.8 经验系数,无法对照 cursor.com)

#### Cursor CSV 新列名兼容 + Included 事件 non-billable
- 现象:2026-05 起 cursor.com 的 `/api/dashboard/export-usage-events-csv` 导出列名变更——`Cost` 列移除,新增 `Cloud Agent ID` / `Automation ID` / `Requests`;同时 `Kind` 出现新值 `Included`(订阅内事件)。`parse_csv` 当前把 Cost 列当必需,缺失时直接 `return Vec::new()`,会让所有 token 解析失败;`is_billable_kind` 把 `Included` 当 billable 累加 cost
- `Cost` 降级为可选列:缺失时 cost=0(`Cursor 计费` 卡显示 $0,token 维度照常解析)。当前 `?strategy=tokens` 接口仍带 Cost 列,但 cursor 后端早晚会跟 dashboard 对齐,提前兼容
- `is_billable_kind` 把 `included` 加入 non-billable(与 `no charge` / `free` 同列),订阅内事件 cost 不再累计

#### Cursor 统计页文案诚实化
- 每日 Token 图说明:"与 cursor.com Dashboard 一致" 改为 "与 cursor.com CSV 用量接口一致;Dashboard 对 thinking 模型有内部加权,可能有 ~5% 偏差,以 CSV 为准"
- "估算费用" 卡片改名 `Cursor 计费 ($)`,悬停 tooltip 说明:"cursor.com CSV 给订阅内 (Included) 事件的内部计费,通常远低于 Anthropic API 真实价格 (~$1/M tokens),也不等于你付的 Cursor 月费"
- `StatCard` 组件新增可选 `hint` prop,带 hint 时 label 末加 ⓘ,整张卡片可悬停看说明

## [0.5.23] - 2026-05-15

### Added

#### Cursor 使用统计接入官方用量接口（cursor.com CSV）
- 现象：Cursor 「使用统计」页此前只读本地 SQLite bubble，缺失 `cache_read` / `cache_write`（在长 Agent 会话里这两项常占 80%+），也没有 cost；展示出来的 token 总量是 cursor.com Dashboard 的一个**小子集**，数字常年偏低，用户看到的成本永远不准
- 新增 `cursor::api::auth` + `cursor::api::usage_csv`：从本机 `state.vscdb` 读 `cursorAuth/accessToken` JWT、`~/.cursor/cli-config.json` 取 userId，拼出 WorkOS session cookie，调用 `https://cursor.com/api/dashboard/export-usage-events-csv?strategy=tokens`，CSV 解析回到结构化数据
- `cursor::commands::stats::get_stats` 接入：成功时 token / cache / cost 来自官方接口（`dataSource: "api"`），失败时把 `auth_status` 暴露给前端（`expired` / `missing` / `network` / `unknown`），同时把所有 token 字段强制清零，避免用户看见误导性的 bubble-only 总数
- 前端 `CursorStatsView`：未登录 / 接口失败时显示红色横幅"请登录 Cursor"提示并把 Token 卡片显示 `—`；登录成功时把 daily token chart 与 cost 一起渲染；项目维度沿用本地 SQLite（仅用于横向对比，文案做了显式区分）
- `reqwest` 启用 `blocking` feature；新增 `base64` 依赖用于解析 JWT payload 取 userId

#### Cursor 高水位历史脏数据一次性迁移
- 0.5.x 之前 cursor 上报把 bubble-only token（只含 input/output、缺 cache）写到了"工作区项目名"下；新代码改为把准确的官方总量挂在特殊项目 `(cursor)` 下，本地工作区行 token 清零
- 但 `report.rs` 的高水位"取 max"语义会把这些历史错误数永远钉住，导致服务端 dashboard 双计
- 新增 `migrate_cursor_hw_2026_05`：首次启动时遍历高水位文件，删除所有非 `(cursor)` 项目的 cursor 行，写一个 flag 文件保证只跑一次；后续上报让服务端 upsert 自然覆盖

### Fixed

#### Claude subagent token 漏算（recursive scan）
- Claude Code 把 Agent / Task 工具调用的 subagent 上下文写在 `projects/<encoded>/subagents/agent-*.jsonl` 子目录里，而旧逻辑只 `read_dir` 项目根，没递归子目录
- 后果：subagent 运行里最大的 `cache_creation_input_tokens` 块（每次重新加载上下文都触发 5m cache write）被完全跳过，重度使用 Task tool 的用户在统计页看到的 token 比真实少一截
- 新增 `path_encoder::list_session_jsonl_files` 递归收集所有 `.jsonl`，`claude::commands::stats` + `report` 全切到新 helper

#### Codex model 字段错位导致服务端定价错算
- Codex JSONL 的 `session_meta` 只有 `model_provider`，真实 model id（`gpt-5.5` / `o4-mini` 等）写在后续 `turn_context` 行里
- 旧代码 `report.rs::collect_codex_records` 把 `model_provider` 当成 model 上报，服务端的 pricing 表匹配不到就走默认 fallback × 0.2，价格全错
- `extract_session_meta` 改为扫前 20 行，session_meta + turn_context 两条记录都解析；优先用 `turn_context.model`，回退到 `model_provider`

#### Codex 历史 AGENTS.md 系统提示混在用户消息里
- Codex CLI 在每个 session 的第一条 user-role 消息位置注入 `# AGENTS.md instructions for <path>` 作为"伪用户消息"喂给模型；这条不是用户输入，但旧 parser 当成真用户消息渲染了
- `jsonl::parse_all_messages` 检测到首条 user 消息以这个固定前缀开头时直接跳过

#### OpenCode 上报维度错配
- 旧 `collect_opencode_records` 在 session 粒度聚合，model 硬编码 "opencode"、token 全 0、跨日 session 全部记到 `time_updated` 当日
- 改为 message 粒度聚合：从 `message.data.tokens.{input,output,cache.read,cache.write}` 读取真实 token，model 用 `providerID/modelID` 拼接，每条 assistant message 落到自己的 `time_created` 当日，session_count 用 HashSet 去重

#### OpenCode 中文 first_prompt 切片 panic
- `get_first_prompt` 用 `&text[..100]` 截断超长 prompt，100 字节边界正好落在中文 UTF-8 字节中段时 panic（Windows release build 直接窗口关闭）
- 修正为 `is_char_boundary` 回退到最近的字符边界后再切

#### OpenCode 会话列表 path 列名修正
- `query_sessions` SQL 选的是 `path`，但当前版本 OpenCode SQLite schema 里这一列叫 `directory`；旧 SQL 在新 OpenCode 上回退到 COALESCE 的空串，会话列表显示不出工作目录
- 改为 `COALESCE(directory, '')`

### Changed

#### Codex / OpenCode 使用统计新增 reasoning_tokens 维度
- 新增 `total_reasoning_tokens` + 日维度 `reasoning_tokens`，从 `tokens.reasoning` 字段读取
- 这部分 token 已经计入 `output_tokens` 内部，新字段只是单独暴露，方便前端展示"思考 token 占输出比例"

#### Cursor CLI stats struct 与 Cursor IDE 对齐
- `cursor_cli::commands::stats` 跟随 `CursorStats` 结构新增 `total_cache_read_tokens` / `total_cache_write_tokens` / `estimated_cost` / `data_source` / `auth_status` / `auth_error` 字段
- Cursor CLI 永远不访问 cursor.com，`auth_status` 固定 `"ok"` 表示"无需鉴权"，token 字段保持 0（CLI 本地也不存 token）

## [0.5.15] - 2026-05-14

### Fixed

#### Claude 使用统计页看不到当前月份的数据
- 现象：客户打开「使用统计」时，月份下拉只能选到过往月份，本月（含本月新写入的 token / message / 工具调用）完全不出现
- 根因：`get_global_stats()` 直接吐 `~/.claude/stats-cache.json`，这份缓存是 Claude Code CLI 自己维护的，最近没用 CLI、或 CLI 后台聚合还没跑时会停在几天～几周前的 `last_computed_date`；前端 `extractMonths` 从返回数据里反推月份列表，本月数据不在 → 月份选项也不出现
- 修复（`src-tauri/src/claude/commands/stats.rs`）：把 `stats-cache.json` 当作 baseline，按 `last_computed_date` 当截断日，扫 `~/.claude/projects/*/*.jsonl` 中 mtime 在截断日之后的文件，把日期 `>` cutoff 的记录增量合并进 `daily_activity` / `daily_model_tokens` / `model_usage`；session_count 只在「文件第一条记录」也晚于 cutoff 时计入，避免与 cache 已计入的跨日 session 双计
- 新增两条单元测试覆盖核心不变式：1) 全新 session 的会话/消息/工具调用全部进入 delta；2) 跨 cutoff 的延续 session 不会被二次计入 session_count，cutoff 前的 token 也不会泄漏
- Codex / opencode / Cursor 的 stats 每次都是实时全量扫源数据，本身就不存在该问题，未改动

## [0.5.14] - 2026-05-08

### Added

#### Cursor CLI 作为独立工具项
- 侧边栏下拉新增 `Cursor CLI` 选项，独立于 `Cursor`（IDE）。CLI 来源 `~/.cursor/chats/*/*/store.db`，IDE 来源 `~/.cursor/chats/composer SQLite + agent-transcripts`，两者数据流不再合并
- 后端新增 `cursor_cli` 模块，提供独立的 projects / sessions / messages / search / stats 命令；复用 `cursor::parser::cli_chats` 数据层
- 使用统计 `/api/report` 上报新增 `cursor_cli` tool，model 字段固定为 `cursor-cli`，便于 dashboard 区分
- 会话上报 `/api/conversations` 上报路径拆分：`cursor_scanner::scan_all` 只覆盖 IDE，新增 `scan_all_cli` 覆盖 CLI，分别用 tool=`cursor` 和 tool=`cursor_cli` 上报
- Dashboard `TOOL_LABELS` 新增 `cursor_cli: 'Cursor CLI'`，「查看问题」按钮白名单同步加入

## [0.5.13] - 2026-04-30

### Fixed

#### Linux 编译失败（历史遗留 — 之前每个 release 的 Ubuntu job 都挂在这）
- `src/copilot/commands/terminal.rs:42` 写法 `vec!["-e", &format!("bash -c '{}'", cmd_str)]` 借用 `format!()` 的临时返回值，立刻被 drop → `error[E0716]: temporary value dropped while borrowed`
- 这只在 Linux target 编译路径触发（macOS/Windows 走别的 cfg 分支），所以本地 mac 开发看不到
- 修复方式跟 claude/codex/opencode 已有的写法一致：把 `format!` 提前 bind 到 `let bash_cmd`，`vec` 里借用变量
- 其余三个工具的 terminal.rs 早就是正确写法，只有 copilot 漏了

## [0.5.12] - 2026-04-30

### Fixed

#### Windows / Linux 编译失败（0.5.11 release 中 Windows 包缺失）
- `RunEvent::Reopen` 是 macOS-only variant，0.5.11 写法在 Windows / Linux 上 `error[E0599]: no variant named Reopen found for enum RunEvent`，导致 GitHub Actions release Windows job 失败
- 用 `#[cfg(target_os = "macos")]` 包住 Reopen 处理 block，闭包参数加 `_` 前缀避免 unused warning
- 行为不变：macOS Dock 点击仍可重新唤回主窗口

## [0.5.11] - 2026-04-30

### Fixed

#### macOS Dock 图标点击无响应
- 0.5.8 加入 close-to-tray 后，关窗只 hide 主窗口；macOS 上 Dock 图标点击会触发 `RunEvent::Reopen`，但我们没监听，导致用户只能从托盘菜单/图标重新打开窗口
- 改用 `Builder::build()` + `app.run(|app, event| ...)` 形式监听 `Reopen` 事件，在没可见窗口时 show + focus + unminimize 主窗口
- macOS 用户现在可以从 Dock 图标重新唤回窗口，符合系统约定

## [0.5.10] - 2026-04-27

### Fixed

#### Cursor 「打开」/ 复制按钮静默失效
- 根因 1：`SessionsPage` 用 `encodeURIComponent(p.cwd) === projectKey` 在 projects 数组里找当前 project，但 React Router 的 `useParams` 已经自动 decode URL 参数 → encoded vs decoded 永远不相等 → `project = undefined` → workDir 拿不到 → handler 在 `if (!workDir) return` 提前退出。codex / copilot / cursor 三个工具都中招
- 根因 2：错误反馈用 `window.alert`，Tauri 2 把它路由到 `tauri-plugin-dialog` 的 message API，capabilities 没授权 → "Command plugin:dialog|message not allowed by ACL" promise rejection 静默吞掉。所以即便 spawn 失败用户也看不到错误
- 修复：去掉 encodeURIComponent，直接 `p.cwd === projectKey`；capabilities 加 `dialog:default`

#### Cursor 全局搜索 crash
- `cursor/commands/search.rs:68` 用 byte slicing `&text[..200]` 截断匹配上下文，中文 prompt 200 byte 边界落在多字节字符中间会 panic
- 改为字符级安全截断 `text.chars().take(200).collect()`，加 4 个单测覆盖 ASCII / 中文 / 长串边界
- Windows release build 没 stderr console，crash 时窗口直接关，是用户体验最大杀手

#### Cursor 用量上报缺 Agent Transcripts 来源
- `report::collect_cursor_records` 之前只读 SQLite Composer，没有把新版 Cursor Agent transcripts 算进 usage_records
- 结果：Agent 模式产生的项目在 dashboard 上**完全没有用量行**，conversation 文件即便存在也没"查看问题"入口
- 现在两条来源都聚合：Composer 提供完整 token+session+message，transcripts 补 session+message（无 token）
- transcripts 项目的 `project` 字段使用 `workspace_encoded`，与 conversation 上报保持一致；服务端 fuzzy 匹配（同步发布）桥接 encoded ↔ basename ↔ underscore/dash 差异

### Changed

#### Cursor 会话操作按钮跨平台
- 之前：Resume 按钮调用未实现的 stub（`Err("Cursor session resume is not yet supported")`），点击无反应；Copy 按钮命令是 `open -a Cursor` macOS only
- 现在：
  - Resume 改名「打开」，调用 `open -a Cursor` (macOS) / `cursor` (Windows/Linux) 在 Cursor IDE 中打开 workspace 目录
  - Copy 命令跨平台：macOS 复制 `open -a Cursor '...'`，Windows/Linux 复制 `cursor "..."`
  - Resume 失败用 `alert()` 显示错误（Windows release build 没 console，console.error 用户看不见）
  - Windows 上 spawn cursor 加 `CREATE_NO_WINDOW`

### Server-side（同步）

#### `/api/conversations/detail` project fuzzy 匹配
- usage_records 来自 SQLite Composer 的 basename（`toolkit`），conversation jsonl 来自 Cursor Agent Transcripts 的 workspace_encoded（`d-project-toolkit`，所有 `_/\` 转为 `-`）—— 同一项目两边名字不一致
- 之前严格相等过滤导致 dashboard "查看问题" 拿不到任何 message
- 现在按 `_` 等价 `-` 的归一化 + `(分隔符){query}` 后缀匹配桥接两种命名

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
