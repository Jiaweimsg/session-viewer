# Session Viewer

<p align="center">
  <img src="src-tauri/icons/icon.png" width="128" height="128" alt="Session Viewer">
</p>

<p align="center">
  <strong>Claude Code & Codex 本地会话记录的可视化浏览器</strong>
</p>

<p align="center">
  <a href="https://github.com/Jiaweimsg/session-viewer/releases">
    <img src="https://img.shields.io/github/v/release/Jiaweimsg/session-viewer?style=flat-square" alt="Release">
  </a>
  <a href="https://github.com/Jiaweimsg/session-viewer/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/Jiaweimsg/session-viewer/build.yml?style=flat-square&label=CI" alt="CI">
  </a>
  <a href="https://github.com/Jiaweimsg/session-viewer/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/Jiaweimsg/session-viewer?style=flat-square" alt="License">
  </a>
</p>

---

一个轻量级桌面应用，支持同时浏览 [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 和 [Codex](https://github.com/openai/codex) 的本地会话记录。左上角一键切换工具，查看不同工具的项目、会话、消息、搜索和使用统计。

> 本项目参照 大鹅群（qq:914736421）[zuoliangyu](https://github.com/zuoliangyu) 的 想法和创意。

## Features

- **双工具支持** — 顶部切换 Claude Code / Codex，数据完全独立
- **项目浏览** — 自动扫描本地会话目录，按最近活跃排序
- **会话列表** — 首条 Prompt、消息数量、Git 分支、创建/修改时间
- **消息详情** — Markdown 渲染 + 语法高亮 + 思考过程/推理过程折叠 + 工具调用/函数调用展示
- **一键 Resume** — 在系统终端中恢复会话继续对话
- **全局搜索** — 跨项目、跨会话全文搜索（Rayon 并行）
- **使用统计** — 每日活动图表、Token 用量趋势、模型分布
- **实时更新** — 文件系统监听，新会话自动刷新
- **跳转底部** — 加载全部消息并直达最后一条

## Data Source

本应用**只读取本地文件**，不联网、不上传任何数据。

| 工具 | 数据目录 |
|------|----------|
| Claude Code | `~/.claude/projects/` |
| Codex | `~/.codex/sessions/` |

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | [Tauri v2](https://v2.tauri.app/) (Rust + WebView) |
| Frontend | React 19 + TypeScript + Vite 6 |
| Styling | Tailwind CSS 3 |
| State | Zustand 5 |
| Markdown | react-markdown + remark-gfm + react-syntax-highlighter |
| Charts | Recharts 2 |
| Icons | Lucide React |
| Date | date-fns 4 |
| File Watch | notify 7 (Rust) |
| Parallel Search | Rayon (Rust) |
| Cache | LRU (Rust) |

## Prerequisites

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/tools/install) >= 1.75

### Platform-specific

**Windows:**
- [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (Windows 10/11 通常已内置)

**macOS:**
- Xcode Command Line Tools: `xcode-select --install`

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

## Development

```bash
git clone https://github.com/Jiaweimsg/session-viewer.git
cd session-viewer

npm install
npx tauri dev
```

## Build

```bash
npx tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`：

| Platform | Output |
|----------|--------|
| Windows | `.msi` + `.exe` (NSIS installer) |
| macOS | `.dmg` + `.app` |
| Linux | `.deb` + `.AppImage` |

## Release

创建 `v*` 格式的 tag 触发 GitHub Actions 多平台自动构建：

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Credits

- 原始项目 [claude-memory-viewer](https://github.com/zuoliangyu/claude-memory-viewer) 和 codex-session-viewer 由 [zuoliangyu](https://github.com/zuoliangyu) 开发

## License

[MIT](LICENSE)
