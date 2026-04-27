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
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import * as api from "../../services/tauriApi";

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
        <ManualReportSection />
        <BlocklistSection />
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
            className={`text-xs flex items-center gap-1.5 ${
              status === "success" ? "text-emerald-500" : "text-destructive"
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
