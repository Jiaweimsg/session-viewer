# 会话详情页 上一条 / 下一条 导航设计

> 日期: 2026-04-17
> 范围: 前端仅改 `src/components/message/MessagesPage.tsx`

## 目标

在会话详情页加入"上一条 / 下一条"会话切换能力，不必返回列表即可浏览同一项目下的相邻会话。

## 需求约定（已和用户确认）

| 维度 | 决定 |
|---|---|
| 范围 | 仅同一项目内切换 |
| 顺序 | 按会话列表当前展示的顺序（与侧边 SessionsPage 列表一致） |
| 入口 | 顶栏按钮 + 键盘快捷键 |
| 快捷键 | `←` 上一条 / `→` 下一条；输入框或文本被选中时不触发 |
| 边界行为 | 到达首/末条时按钮 disabled，快捷键 no-op（不循环、不提示） |
| 工具支持 | Claude / Codex / Copilot / Cursor（4 种扁平列表）；OpenCode 不渲染、不响应 |

## 架构

- 单文件改动：`src/components/message/MessagesPage.tsx`
- 零改动：后端 Rust、`appStore`、路由表、其它组件
- 数据源复用：`useAppStore().sessions`（进入 `SessionsPage` 时由 `selectProject` 填充，跨页保留）
- 导航机制：`navigate(newUrl)` 更新 URL，现有 `useEffect([sessionKey, projectKey])` 自动触发 `selectSession` 拉取新消息

## UI

顶栏布局（现有 `←` 返回按钮之后、标题之前插入）：

```
[←]  [‹ 上一条] [› 下一条]   标题 / N 条消息    [Resume] [Copy]
```

- 图标：`ChevronLeft` / `ChevronRight`（与快捷键 `←/→` 语义对齐）
- disabled 状态：`opacity-50 cursor-not-allowed`，`title` 提示 "已到首条" / "已到末条"
- 正常态 `title`：`"上一条 (←)"` / `"下一条 (→)"`
- OpenCode 下整组不渲染

## 核心逻辑

```ts
// 复用 tool-specific session 匹配
const matchesCurrent = (s: any) =>
  activeTool === "codex"
    ? encodeURIComponent(s.filePath) === sessionKey
    : s.sessionId === sessionKey;

// 同文件内小工具函数，不跨文件共享
const getNavKey = (s: any) =>
  activeTool === "codex" ? encodeURIComponent(s.filePath) : s.sessionId;

const currentIndex = sessions.findIndex(matchesCurrent);
const prevSession = currentIndex > 0 ? sessions[currentIndex - 1] : null;
const nextSession =
  currentIndex >= 0 && currentIndex < sessions.length - 1
    ? sessions[currentIndex + 1]
    : null;

const gotoSession = (s: any) => {
  navigate(
    `/${activeTool}/projects/${encodeURIComponent(projectKey!)}/session/${getNavKey(s)}`
  );
};
```

## 快捷键

- `useEffect` 内 `window.addEventListener('keydown', handler)`，卸载时移除
- 触发条件（全部满足）：
  1. `e.key === 'ArrowLeft'` 或 `'ArrowRight'`
  2. 无 `Cmd/Ctrl/Alt/Shift` 修饰键
  3. `e.target` 不匹配 `input, textarea, [contenteditable="true"]`
  4. `window.getSelection()?.toString().length === 0`（未选中任何文本）
  5. `activeTool !== 'opencode'`
- 满足 → `e.preventDefault()` 后调用对应 `gotoSession(prev/next)`；目标为 `null` 则直接 return

## 边界处理

| 场景 | 行为 |
|---|---|
| `sessions.length === 0`（深链冷启动） | 两按钮 disabled，快捷键 no-op |
| 当前 session 不在列表（`findIndex === -1`） | 两按钮 disabled，快捷键 no-op |
| 首条按"上一条" / 末条按"下一条" | 对应按钮 disabled，快捷键 no-op |
| 焦点在 Resume/Copy 按钮上按 `←/→` | 按钮不是可编辑元素 → 触发跳转（可接受） |
| OpenCode | 不渲染按钮、快捷键不响应 |
| 文件系统监听刷新 sessions | `findIndex` 自动重算，按钮状态跟随更新 |

## 测试策略

项目无前端测试框架，采用手动验证 + `npm run build` 类型检查：

1. Claude / Codex / Copilot / Cursor 四种工具各跑一遍 prev/next（点击 + `←/→`）
2. 到首条/末条按钮灰化、快捷键不响应
3. 选中文本后按 `←/→` → 不跳转
4. 输入框防御：当前页面无输入框，该过滤条件作为防御代码存在，确保未来加入搜索框/评论框时自动不触发
5. OpenCode 下按钮不显示、快捷键不响应
6. `npm run build` 无 TS 错误

## 显式排除（YAGNI）

- 不做循环导航
- 不做 toast/动画过渡
- 不抽 store action（索引计算仅 3 行，过度抽象）
- 不改路由形状、不加 index 参数
- 不支持 OpenCode（后续可按"分组铺平"扩展）
- 不重构既有 `session` / `project` 查找逻辑
