import { useEffect, useState } from "react";
import {
  Trash2,
  Plus,
  Save,
  Loader2,
  Check,
  AlertCircle,
  ShieldOff,
  Upload,
  Server,
  FolderOpen,
  UserCircle2,
  RotateCcw,
  RefreshCw,
  Info,
  Github,
  Download,
} from "lucide-react";
import pkg from "../../../package.json";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import * as api from "../../services/tauriApi";
import { checkForUpdate, installUpdate, type Update } from "../../services/updater";

type SaveState =
  | { kind: "idle" }
  | { kind: "saving" }
  | { kind: "ok" }
  | { kind: "err"; msg: string };

const REPORT_SERVER_KEY = "report_server_url";
const DEFAULT_REPORT_SERVER = "http://172.36.164.85:3000";

export function SettingsPage() {
  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-foreground mb-2">设置</h1>
          <p className="text-sm text-muted-foreground">
            管理客户端上报行为。修改保存后，下一轮上报周期（最长 5 分钟）即生效。
          </p>
        </div>
        <ReportServerSection />
        <IdentitySection />
        <ManualReportSection />
        <BlocklistSection />
        <AdvancedSection />
        <AboutSection />
      </div>
    </div>
  );
}

// ============ 服务端地址 ============

function ReportServerSection() {
  const [serverUrl, setServerUrl] = useState(
    () => localStorage.getItem(REPORT_SERVER_KEY) || DEFAULT_REPORT_SERVER
  );
  const [saved, setSaved] = useState<"idle" | "ok">("idle");

  function save() {
    const v = serverUrl.trim();
    if (!v) return;
    localStorage.setItem(REPORT_SERVER_KEY, v);
    setSaved("ok");
    setTimeout(() => setSaved("idle"), 2000);
  }

  function reset() {
    setServerUrl(DEFAULT_REPORT_SERVER);
    localStorage.setItem(REPORT_SERVER_KEY, DEFAULT_REPORT_SERVER);
    setSaved("ok");
    setTimeout(() => setSaved("idle"), 2000);
  }

  return (
    <section className="bg-card border border-border rounded-lg p-5">
      <header className="flex items-center gap-2 mb-1">
        <Server className="w-4 h-4 text-muted-foreground" />
        <h2 className="text-base font-medium text-foreground">上报服务端地址</h2>
      </header>
      <p className="text-sm text-muted-foreground mb-4">
        手动上报时使用的服务端 URL。后台自动上报使用编译时常量，此处仅影响下方"立即上报"按钮。
      </p>
      <div className="flex gap-2">
        <input
          type="text"
          value={serverUrl}
          onChange={(e) => setServerUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") save();
          }}
          placeholder={DEFAULT_REPORT_SERVER}
          className="flex-1 bg-muted text-foreground text-sm rounded-md px-3 py-2 border border-border focus:outline-none focus:ring-2 focus:ring-ring font-mono"
        />
        <button
          onClick={save}
          disabled={!serverUrl.trim()}
          className="flex items-center gap-1.5 bg-primary text-primary-foreground text-sm font-medium rounded-md px-4 py-2 hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <Save className="w-4 h-4" />
          保存
        </button>
        <button
          onClick={reset}
          className="text-sm text-muted-foreground rounded-md px-3 py-2 border border-border hover:bg-accent transition-colors"
          title="恢复默认地址"
        >
          重置
        </button>
      </div>
      <div className="mt-2 h-5 text-xs flex items-center gap-1.5">
        {saved === "ok" && (
          <>
            <Check className="w-3.5 h-3.5 text-emerald-500" />
            <span className="text-emerald-500">已保存</span>
          </>
        )}
      </div>
    </section>
  );
}

// ============ 用户身份 ============

function IdentitySection() {
  const [view, setView] = useState<api.IdentityView | null>(null);
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [save, setSave] = useState<SaveState>({ kind: "idle" });

  async function refresh() {
    try {
      const v = await api.getIdentityView();
      setView(v);
      setName(v.override_name ?? "");
      setEmail(v.override_email ?? "");
    } catch (e: any) {
      setSave({ kind: "err", msg: String(e?.message ?? e) });
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  async function persist() {
    setSave({ kind: "saving" });
    try {
      await api.setIdentityOverride({
        user_name: name.trim() || null,
        user_email: email.trim() || null,
      });
      setSave({ kind: "ok" });
      await refresh();
      setTimeout(
        () => setSave((s) => (s.kind === "ok" ? { kind: "idle" } : s)),
        2000
      );
    } catch (e: any) {
      setSave({ kind: "err", msg: String(e?.message ?? e) });
    }
  }

  async function reset() {
    setName("");
    setEmail("");
    setSave({ kind: "saving" });
    try {
      await api.setIdentityOverride({ user_name: null, user_email: null });
      setSave({ kind: "ok" });
      await refresh();
      setTimeout(
        () => setSave((s) => (s.kind === "ok" ? { kind: "idle" } : s)),
        2000
      );
    } catch (e: any) {
      setSave({ kind: "err", msg: String(e?.message ?? e) });
    }
  }

  function defaultSourceLabel(kind: "name" | "email"): string {
    if (!view) return "";
    if (kind === "name") {
      if (view.git_name) return `Git: ${view.git_name}`;
      return `OS 用户名: ${view.os_user}`;
    }
    if (view.git_email) return `Git: ${view.git_email}`;
    return `${view.os_user}@${view.hostname}.local`;
  }

  return (
    <section className="bg-card border border-border rounded-lg p-5">
      <header className="flex items-center gap-2 mb-1">
        <UserCircle2 className="w-4 h-4 text-muted-foreground" />
        <h2 className="text-base font-medium text-foreground">用户身份</h2>
      </header>
      <p className="text-sm text-muted-foreground mb-4">
        当前上报使用的姓名和邮箱。默认从 Git 全局配置读取，没有则用计算机名兜底。
        如果显示不准确，可在下方手动订正；保存后下一轮上报立即生效。
      </p>

      {view && (
        <div className="text-xs text-muted-foreground bg-muted/50 rounded-md px-3 py-2 mb-4">
          目前会上报为
          <span className="font-mono text-foreground mx-1">
            {view.effective_name}
          </span>
          &lt;
          <span className="font-mono text-foreground">{view.effective_email}</span>
          &gt;
        </div>
      )}

      <div className="space-y-3">
        <div>
          <label className="text-xs text-muted-foreground block mb-1">
            姓名（留空则使用：{defaultSourceLabel("name")}）
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={defaultSourceLabel("name")}
            className="w-full bg-muted text-foreground text-sm rounded-md px-3 py-2 border border-border focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>
        <div>
          <label className="text-xs text-muted-foreground block mb-1">
            邮箱（留空则使用：{defaultSourceLabel("email")}）
          </label>
          <input
            type="text"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder={defaultSourceLabel("email")}
            className="w-full bg-muted text-foreground text-sm rounded-md px-3 py-2 border border-border focus:outline-none focus:ring-2 focus:ring-ring font-mono"
          />
          <div className="text-xs text-muted-foreground mt-1">
            服务端按邮箱识别用户。改邮箱会被视作新身份，历史用量不会迁移。
          </div>
        </div>
      </div>

      <div className="flex items-center gap-2 mt-4">
        <button
          onClick={persist}
          disabled={save.kind === "saving"}
          className="flex items-center gap-1.5 bg-primary text-primary-foreground text-sm font-medium rounded-md px-4 py-2 hover:bg-primary/90 disabled:opacity-50 transition-colors"
        >
          <Save className="w-4 h-4" />
          保存
        </button>
        <button
          onClick={reset}
          disabled={save.kind === "saving" || (!view?.override_name && !view?.override_email && !name && !email)}
          className="flex items-center gap-1.5 text-sm text-muted-foreground rounded-md px-3 py-2 border border-border hover:bg-accent hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="清除订正，使用默认值"
        >
          <RotateCcw className="w-4 h-4" />
          重置
        </button>
        <div className="ml-auto h-5 text-xs flex items-center gap-1.5">
          {save.kind === "saving" && (
            <span className="text-muted-foreground">保存中…</span>
          )}
          {save.kind === "ok" && (
            <>
              <Check className="w-3.5 h-3.5 text-emerald-500" />
              <span className="text-emerald-500">已保存</span>
            </>
          )}
          {save.kind === "err" && (
            <>
              <AlertCircle className="w-3.5 h-3.5 text-destructive" />
              <span className="text-destructive">{save.msg}</span>
            </>
          )}
        </div>
      </div>
    </section>
  );
}

// ============ 手动上报 ============

function ManualReportSection() {
  const [status, setStatus] = useState<"idle" | "loading" | "success" | "error">(
    "idle"
  );
  const [message, setMessage] = useState("");

  async function handleReport() {
    const serverUrl = (
      localStorage.getItem(REPORT_SERVER_KEY) || DEFAULT_REPORT_SERVER
    ).trim();
    if (!serverUrl) {
      setStatus("error");
      setMessage("请先在上方配置服务端地址");
      return;
    }
    setStatus("loading");
    setMessage("");
    try {
      const resp = await api.reportUsage(serverUrl);
      if (resp.ok) {
        setStatus("success");
        setMessage(`上报成功，共 ${resp.received ?? 0} 条记录`);
        setTimeout(() => {
          setStatus("idle");
          setMessage("");
        }, 3000);
      } else {
        setStatus("error");
        setMessage(resp.error || "上报失败");
        setTimeout(() => {
          setStatus("idle");
          setMessage("");
        }, 5000);
      }
    } catch (e: any) {
      setStatus("error");
      setMessage(String(e?.message ?? e));
      setTimeout(() => {
        setStatus("idle");
        setMessage("");
      }, 5000);
    }
  }

  return (
    <section className="bg-card border border-border rounded-lg p-5">
      <header className="flex items-center gap-2 mb-1">
        <Upload className="w-4 h-4 text-muted-foreground" />
        <h2 className="text-base font-medium text-foreground">手动上报</h2>
      </header>
      <p className="text-sm text-muted-foreground mb-4">
        立即将本机所有工具的用量数据上传到服务端。后台会每 5 分钟自动上报，此处用于排查或强制同步。
      </p>
      <div className="flex items-center gap-3">
        <button
          onClick={handleReport}
          disabled={status === "loading"}
          className="flex items-center gap-1.5 bg-primary text-primary-foreground text-sm font-medium rounded-md px-4 py-2 hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {status === "loading" ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <Upload className="w-4 h-4" />
          )}
          {status === "loading" ? "上报中…" : "立即上报"}
        </button>
        {status !== "idle" && status !== "loading" && message && (
          <span
            className={`text-xs flex items-center gap-1.5 ${status === "success" ? "text-emerald-500" : "text-destructive"
              }`}
          >
            {status === "success" ? (
              <Check className="w-3.5 h-3.5" />
            ) : (
              <AlertCircle className="w-3.5 h-3.5" />
            )}
            {message}
          </span>
        )}
      </div>
    </section>
  );
}

// ============ 上报黑名单 ============

function BlocklistSection() {
  const [prefixes, setPrefixes] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [loading, setLoading] = useState(true);
  const [saveState, setSaveState] = useState<SaveState>({ kind: "idle" });

  useEffect(() => {
    api
      .getUploadBlocklist()
      .then((b) => setPrefixes(b.cwd_prefixes ?? []))
      .catch(() => setPrefixes([]))
      .finally(() => setLoading(false));
  }, []);

  const dirty = saveState.kind !== "saving";

  async function persist(next: string[]) {
    setSaveState({ kind: "saving" });
    try {
      await api.setUploadBlocklist({ cwd_prefixes: next });
      setSaveState({ kind: "ok" });
      setTimeout(
        () => setSaveState((s) => (s.kind === "ok" ? { kind: "idle" } : s)),
        2000
      );
    } catch (e: any) {
      setSaveState({ kind: "err", msg: String(e?.message ?? e) });
    }
  }

  function add() {
    const v = draft.trim();
    if (!v) return;
    if (prefixes.includes(v)) {
      setDraft("");
      return;
    }
    const next = [...prefixes, v];
    setPrefixes(next);
    setDraft("");
    persist(next);
  }

  async function pickFolder() {
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "选择要加入黑名单的文件夹",
      });
      if (typeof selected === "string" && selected.trim()) {
        const v = selected.trim();
        if (prefixes.includes(v)) return;
        const next = [...prefixes, v];
        setPrefixes(next);
        persist(next);
      }
    } catch (e: any) {
      setSaveState({ kind: "err", msg: String(e?.message ?? e) });
    }
  }

  function remove(i: number) {
    const next = prefixes.filter((_, idx) => idx !== i);
    setPrefixes(next);
    persist(next);
  }

  return (
    <section className="bg-card border border-border rounded-lg p-5">
      <header className="flex items-center gap-2 mb-1">
        <ShieldOff className="w-4 h-4 text-muted-foreground" />
        <h2 className="text-base font-medium text-foreground">
          对话内容上报黑名单
        </h2>
      </header>
      <p className="text-sm text-muted-foreground mb-4">
        列表中的目录及其子目录，<strong>问题内容（用户 Prompt）不会上报</strong>；
        <span className="text-foreground">用量统计（Token / 会话数）照常上报</span>。
        匹配规则：cwd 等于该路径或位于其子树。
      </p>

      <div className="flex gap-2 mb-4">
        <input
          type="text"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") add();
          }}
          placeholder="/Users/you/work/secret-project"
          className="flex-1 bg-muted text-foreground text-sm rounded-md px-3 py-2 border border-border focus:outline-none focus:ring-2 focus:ring-ring font-mono"
        />
        <button
          onClick={pickFolder}
          disabled={!dirty}
          className="flex items-center gap-1.5 text-sm text-muted-foreground rounded-md px-3 py-2 border border-border hover:bg-accent hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="从文件夹选择器添加"
        >
          <FolderOpen className="w-4 h-4" />
          选择文件夹
        </button>
        <button
          onClick={add}
          disabled={!draft.trim() || !dirty}
          className="flex items-center gap-1.5 bg-primary text-primary-foreground text-sm font-medium rounded-md px-4 py-2 hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <Plus className="w-4 h-4" />
          添加
        </button>
      </div>

      {loading ? (
        <div className="flex items-center gap-2 py-4 text-sm text-muted-foreground">
          <Loader2 className="w-4 h-4 animate-spin" />
          加载中…
        </div>
      ) : prefixes.length === 0 ? (
        <div className="text-sm text-muted-foreground py-4 border border-dashed border-border rounded-md text-center">
          暂无黑名单。所有项目的对话内容都会上报。
        </div>
      ) : (
        <ul className="space-y-1">
          {prefixes.map((p, i) => (
            <li
              key={`${p}-${i}`}
              className="flex items-center gap-2 bg-muted/50 rounded-md px-3 py-2 group"
            >
              <span className="font-mono text-sm text-foreground flex-1 truncate" title={p}>
                {p}
              </span>
              <button
                onClick={() => remove(i)}
                className="p-1 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors opacity-0 group-hover:opacity-100"
                title="移除"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="mt-4 h-5 text-xs flex items-center gap-1.5">
        {saveState.kind === "saving" && (
          <>
            <Save className="w-3.5 h-3.5 animate-pulse text-muted-foreground" />
            <span className="text-muted-foreground">保存中…</span>
          </>
        )}
        {saveState.kind === "ok" && (
          <>
            <Check className="w-3.5 h-3.5 text-emerald-500" />
            <span className="text-emerald-500">已保存</span>
          </>
        )}
        {saveState.kind === "err" && (
          <>
            <AlertCircle className="w-3.5 h-3.5 text-destructive" />
            <span className="text-destructive">保存失败：{saveState.msg}</span>
          </>
        )}
      </div>
    </section>
  );
}

// ============ 高级（重置/排错） ============

function AdvancedSection() {
  const [sourceDirs, setSourceDirs] = useState<string[]>([]);
  const [draftDir, setDraftDir] = useState("");
  const [sourceLoading, setSourceLoading] = useState(true);
  const [sourceSave, setSourceSave] = useState<SaveState>({ kind: "idle" });
  const [resetBusy, setResetBusy] = useState(false);
  const [resetMsg, setResetMsg] = useState<{ kind: "ok" | "err" | "idle"; text?: string }>({
    kind: "idle",
  });
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    api
      .getScanDirs()
      .then((dirs) => setSourceDirs(dirs.paths ?? []))
      .catch((e: any) =>
        setSourceSave({ kind: "err", msg: String(e?.message ?? e) })
      )
      .finally(() => setSourceLoading(false));
  }, []);

  const sourceReady = sourceSave.kind !== "saving" && !sourceLoading;

  async function persistSourceDirs(next: string[]) {
    setSourceSave({ kind: "saving" });
    try {
      await api.setScanDirs({ paths: next });
      setSourceSave({ kind: "ok" });
      setTimeout(
        () => setSourceSave((s) => (s.kind === "ok" ? { kind: "idle" } : s)),
        2000
      );
    } catch (e: any) {
      setSourceSave({ kind: "err", msg: String(e?.message ?? e) });
    }
  }

  function addSourceDir() {
    const v = draftDir.trim();
    if (!v) return;
    if (sourceDirs.includes(v)) {
      setDraftDir("");
      return;
    }
    const next = [...sourceDirs, v];
    setSourceDirs(next);
    setDraftDir("");
    persistSourceDirs(next);
  }

  async function pickSourceDir() {
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "选择 Claude Code 源账号目录",
      });
      if (typeof selected === "string" && selected.trim()) {
        const v = selected.trim();
        if (sourceDirs.includes(v)) return;
        const next = [...sourceDirs, v];
        setSourceDirs(next);
        persistSourceDirs(next);
      }
    } catch (e: any) {
      setSourceSave({ kind: "err", msg: String(e?.message ?? e) });
    }
  }

  function removeSourceDir(i: number) {
    const next = sourceDirs.filter((_, idx) => idx !== i);
    setSourceDirs(next);
    persistSourceDirs(next);
  }

  async function reset() {
    setResetBusy(true);
    setResetMsg({ kind: "idle" });
    try {
      await api.resetConversationState();
      setResetMsg({
        kind: "ok",
        text: "已重置。下一轮上报（≤5 分钟）会重新扫描并上传全部历史。",
      });
      setConfirming(false);
    } catch (e: any) {
      setResetMsg({ kind: "err", text: String(e?.message ?? e) });
    } finally {
      setResetBusy(false);
    }
  }

  return (
    <section className="bg-card border border-border rounded-lg p-5">
      <header className="flex items-center gap-2 mb-1">
        <RefreshCw className="w-4 h-4 text-muted-foreground" />
        <h2 className="text-base font-medium text-foreground">高级</h2>
      </header>
      <p className="text-sm text-muted-foreground mb-4">
        配置 Claude Code 多账号源目录，并提供上报状态重置。
      </p>

      <div className="border-b border-border pb-5 mb-5">
        <header className="flex items-center gap-2 mb-1">
          <FolderOpen className="w-4 h-4 text-muted-foreground" />
          <h3 className="text-sm font-medium text-foreground">Claude Code 源账号目录</h3>
        </header>
        <p className="text-sm text-muted-foreground mb-4">
          默认扫描 <span className="font-mono text-foreground">~/.claude</span>。
          多账号时可添加额外账号根目录，例如
          <span className="font-mono text-foreground mx-1">/Users/bin/.claude-cc-bin</span>
          ，也可直接添加其 <span className="font-mono text-foreground">projects</span> 子目录。
        </p>

        <div className="flex gap-2 mb-4">
          <input
            type="text"
            value={draftDir}
            onChange={(e) => setDraftDir(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") addSourceDir();
            }}
            placeholder="/Users/bin/.claude-cc-bin"
            className="flex-1 bg-muted text-foreground text-sm rounded-md px-3 py-2 border border-border focus:outline-none focus:ring-2 focus:ring-ring font-mono"
          />
          <button
            onClick={pickSourceDir}
            disabled={!sourceReady}
            className="flex items-center gap-1.5 text-sm text-muted-foreground rounded-md px-3 py-2 border border-border hover:bg-accent hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title="从文件夹选择器添加"
          >
            <FolderOpen className="w-4 h-4" />
            选择文件夹
          </button>
          <button
            onClick={addSourceDir}
            disabled={!draftDir.trim() || !sourceReady}
            className="flex items-center gap-1.5 bg-primary text-primary-foreground text-sm font-medium rounded-md px-4 py-2 hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            <Plus className="w-4 h-4" />
            添加
          </button>
        </div>

        {sourceLoading ? (
          <div className="flex items-center gap-2 py-4 text-sm text-muted-foreground">
            <Loader2 className="w-4 h-4 animate-spin" />
            加载中…
          </div>
        ) : sourceDirs.length === 0 ? (
          <div className="text-sm text-muted-foreground py-4 border border-dashed border-border rounded-md text-center">
            暂无额外源目录。当前只扫描默认 ~/.claude。
          </div>
        ) : (
          <ul className="space-y-1">
            {sourceDirs.map((p, i) => (
              <li
                key={`${p}-${i}`}
                className="flex items-center gap-2 bg-muted/50 rounded-md px-3 py-2 group"
              >
                <span className="font-mono text-sm text-foreground flex-1 truncate" title={p}>
                  {p}
                </span>
                <button
                  onClick={() => removeSourceDir(i)}
                  disabled={!sourceReady}
                  className="p-1 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors opacity-0 group-hover:opacity-100 disabled:opacity-50 disabled:cursor-not-allowed"
                  title="移除"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </li>
            ))}
          </ul>
        )}

        <div className="mt-4 h-5 text-xs flex items-center gap-1.5">
          {sourceSave.kind === "saving" && (
            <>
              <Save className="w-3.5 h-3.5 animate-pulse text-muted-foreground" />
              <span className="text-muted-foreground">保存中…</span>
            </>
          )}
          {sourceSave.kind === "ok" && (
            <>
              <Check className="w-3.5 h-3.5 text-emerald-500" />
              <span className="text-emerald-500">已保存</span>
            </>
          )}
          {sourceSave.kind === "err" && (
            <>
              <AlertCircle className="w-3.5 h-3.5 text-destructive" />
              <span className="text-destructive">保存失败：{sourceSave.msg}</span>
            </>
          )}
        </div>
      </div>

      <div>
        <header className="flex items-center gap-2 mb-1">
          <RefreshCw className="w-4 h-4 text-muted-foreground" />
          <h3 className="text-sm font-medium text-foreground">对话上报状态</h3>
        </header>
        <p className="text-sm text-muted-foreground mb-4">
          当 dashboard 看不到对话内容、但用量正常时，重置上报状态可让客户端
          重新扫描所有历史 jsonl 并重新上传。服务端按 uuid 去重，重复消息不会落盘。
        </p>
        <div className="flex items-center gap-2">
          {confirming ? (
            <>
              <button
                onClick={reset}
                disabled={resetBusy}
                className="flex items-center gap-1.5 bg-destructive text-destructive-foreground text-sm font-medium rounded-md px-4 py-2 hover:bg-destructive/90 disabled:opacity-50 transition-colors"
              >
                {resetBusy ? <Loader2 className="w-4 h-4 animate-spin" /> : <RefreshCw className="w-4 h-4" />}
                确认重置
              </button>
              <button
                onClick={() => setConfirming(false)}
                disabled={resetBusy}
                className="text-sm text-muted-foreground rounded-md px-3 py-2 border border-border hover:bg-accent transition-colors disabled:opacity-50"
              >
                取消
              </button>
            </>
          ) : (
            <button
              onClick={() => setConfirming(true)}
              className="flex items-center gap-1.5 text-sm text-foreground rounded-md px-4 py-2 border border-border hover:bg-accent transition-colors"
            >
              <RefreshCw className="w-4 h-4" />
              重置对话上报状态
            </button>
          )}
          <div className="ml-auto h-5 text-xs flex items-center gap-1.5">
          {resetMsg.kind === "ok" && (
            <>
              <Check className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
              <span className="text-emerald-500">{resetMsg.text}</span>
            </>
          )}
          {resetMsg.kind === "err" && (
            <>
              <AlertCircle className="w-3.5 h-3.5 text-destructive shrink-0" />
              <span className="text-destructive">{resetMsg.text}</span>
            </>
          )}
          </div>
        </div>
      </div>
    </section>
  );
}

// ============ 关于 ============

const RELEASES_URL = "https://github.com/Jiaweimsg/session-viewer/releases";

type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "latest" }
  | { kind: "available"; version: string; notes?: string; update: Update }
  | { kind: "downloading"; version: string; fraction: number | null }
  | { kind: "error"; message: string };

function AboutSection() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  const openLink = async (url: string) => {
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(url);
    } catch (e) {
      console.error(e);
      window.open(url, '_blank');
    }
  };

  const handleCheck = async () => {
    setState({ kind: "checking" });
    try {
      const update = await checkForUpdate();
      if (update) {
        setState({
          kind: "available",
          version: update.version,
          notes: update.body || undefined,
          update,
        });
      } else {
        setState({ kind: "latest" });
      }
    } catch (e: any) {
      setState({ kind: "error", message: e?.message || String(e) || "检查失败" });
    }
  };

  const handleInstall = async (update: Update, version: string) => {
    setState({ kind: "downloading", version, fraction: 0 });
    try {
      // 安装完成后会自动重启，正常不会返回到这里
      await installUpdate(update, (p) => {
        setState({ kind: "downloading", version, fraction: p.fraction });
      });
    } catch (e: any) {
      setState({ kind: "error", message: e?.message || String(e) || "更新失败" });
    }
  };

  const busy = state.kind === "checking" || state.kind === "downloading";

  return (
    <section className="bg-card border border-border rounded-lg p-5">
      <header className="flex items-center gap-2 mb-4">
        <Info className="w-4 h-4 text-muted-foreground" />
        <h2 className="text-base font-medium text-foreground">关于</h2>
      </header>
      <div className="flex flex-col gap-3 text-sm">
        <div className="flex items-center justify-between py-2 border-b border-border/50">
          <span className="text-muted-foreground">当前版本</span>
          <div className="flex items-center gap-3">
            <span className="font-mono text-foreground font-medium">v{pkg.version}</span>
            <button
              onClick={handleCheck}
              disabled={busy}
              className="flex items-center gap-1.5 text-xs bg-muted text-foreground px-2 py-1 rounded hover:bg-accent transition-colors disabled:opacity-50"
            >
              {busy ? <Loader2 className="w-3 h-3 animate-spin" /> : <RefreshCw className="w-3 h-3" />}
              {state.kind === "checking"
                ? "检查中..."
                : state.kind === "downloading"
                  ? "更新中..."
                  : "检查更新"}
            </button>
          </div>
        </div>

        {state.kind === "error" && (
          <div className="py-2 border-b border-border/50">
            <div className="flex items-center justify-between bg-destructive/10 text-destructive px-3 py-2 rounded-md gap-3">
              <span className="text-xs flex items-center gap-1 min-w-0">
                <AlertCircle className="w-3 h-3 shrink-0" />
                <span className="truncate">更新失败: {state.message}</span>
              </span>
              <button
                onClick={() => openLink(RELEASES_URL)}
                className="text-xs font-medium underline hover:opacity-80 shrink-0"
              >
                手动下载
              </button>
            </div>
          </div>
        )}

        {state.kind === "latest" && (
          <div className="py-2 border-b border-border/50">
            <span className="text-muted-foreground text-xs flex items-center gap-1">
              <Check className="w-3 h-3" />
              当前已是最新版本
            </span>
          </div>
        )}

        {state.kind === "available" && (
          <div className="py-2 border-b border-border/50">
            <div className="flex items-center justify-between bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 px-3 py-2 rounded-md gap-3">
              <span className="text-xs">发现新版本: v{state.version}</span>
              <button
                onClick={() => handleInstall(state.update, state.version)}
                className="flex items-center gap-1.5 text-xs font-medium bg-emerald-500 text-white px-2.5 py-1 rounded hover:bg-emerald-600 transition-colors"
              >
                <Download className="w-3 h-3" />
                下载并安装
              </button>
            </div>
            {state.notes && (
              <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground whitespace-pre-line max-h-24 overflow-y-auto">
                {state.notes}
              </p>
            )}
          </div>
        )}

        {state.kind === "downloading" && (
          <div className="py-2 border-b border-border/50">
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-xs text-muted-foreground">
                正在下载 v{state.version}…
              </span>
              {state.fraction != null && (
                <span className="text-xs font-mono text-muted-foreground">
                  {Math.round(state.fraction * 100)}%
                </span>
              )}
            </div>
            <div className="h-1.5 w-full bg-muted rounded-full overflow-hidden">
              <div
                className={
                  state.fraction != null
                    ? "h-full bg-emerald-500 transition-[width] duration-200"
                    : "h-full bg-emerald-500 animate-pulse w-1/3"
                }
                style={state.fraction != null ? { width: `${state.fraction * 100}%` } : undefined}
              />
            </div>
            <p className="mt-1.5 text-[11px] text-muted-foreground">
              下载完成后将自动安装并重启应用，请勿手动关闭。
            </p>
          </div>
        )}

        <div className="flex items-center justify-between py-2">
          <span className="text-muted-foreground">代码仓库</span>
          <button
            onClick={() => openLink("https://github.com/Jiaweimsg/session-viewer")}
            className="flex items-center gap-1.5 text-primary hover:text-primary/80 transition-colors"
          >
            <Github className="w-4 h-4" />
            <span className="font-mono">Jiaweimsg/session-viewer</span>
          </button>
        </div>
      </div>
    </section>
  );
}
