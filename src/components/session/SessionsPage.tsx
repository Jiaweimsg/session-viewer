import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import {
  ArrowLeft,
  MessageSquare,
  Clock,
  GitBranch,
  Play,
  Copy,
  Check,
} from "lucide-react";
import { formatDistanceToNow, format } from "date-fns";
import { zhCN } from "date-fns/locale";
import { resumeSession } from "../../services/tauriApi";
import { OpencodeSessionList } from "./OpencodeSessionList";
import type { OpencodeSessionGroup } from "../../types";

export function SessionsPage() {
  const { projectKey } = useParams<{ tool: string; projectKey: string }>();
  const navigate = useNavigate();
  const {
    activeTool,
    sessions,
    sessionsLoading,
    selectProject,
    projects,
  } = useAppStore();

  const [copiedId, setCopiedId] = useState<string | null>(null);

  const project =
    activeTool === "codex"
      ? projects.find((p) => p.cwd === projectKey)
      : activeTool === "copilot"
      ? projects.find((p) => p.cwd === projectKey)
      : activeTool === "cursor" || activeTool === "cursor-cli"
      ? projects.find((p) => p.cwd === projectKey)
      : projects.find((p) => p.encodedName === projectKey);

  useEffect(() => {
    if (projectKey) {
      selectProject(projectKey);
    }
  }, [projectKey]);

  const handleResume = async (
    e: React.MouseEvent,
    session: any
  ) => {
    e.stopPropagation();
    const workDir =
      activeTool === "codex"
        ? session.cwd
        : activeTool === "copilot"
        ? session.cwd || project?.cwd || null
        : activeTool === "cursor" || activeTool === "cursor-cli"
        ? project?.cwd || null
        : session.projectPath || project?.displayPath || null;
    if (!workDir) return;
    try {
      await resumeSession(activeTool, session.sessionId, workDir, session.fullPath || undefined);
    } catch (err) {
      console.error("Failed to resume session:", err);
      const msg = typeof err === "string" ? err : (err as any)?.message ?? String(err);
      alert(msg);
    }
  };

  const handleCopy = (e: React.MouseEvent, session: any) => {
    e.stopPropagation();
    const workDir =
      activeTool === "codex"
        ? session.cwd
        : activeTool === "copilot"
        ? session.cwd || project?.cwd || null
        : activeTool === "cursor" || activeTool === "cursor-cli"
        ? project?.cwd || null
        : session.projectPath || project?.displayPath || null;
    if (!workDir) return;
    const isWindows =
      typeof navigator !== "undefined" &&
      /win/i.test(navigator.platform);
    const cmd =
      activeTool === "codex"
        ? `cd '${workDir}' && codex resume ${session.sessionId}`
        : activeTool === "copilot"
        ? `cd '${workDir}' && copilot --resume=${session.sessionId}`
        : activeTool === "cursor" || activeTool === "cursor-cli"
        // Cursor: macOS 用 open -a Cursor（不依赖 PATH 上的 cursor CLI），
        // Windows/Linux 用 cursor 命令（IDE 安装时默认入 PATH）
        ? (isWindows ? `cursor "${workDir}"` : `open -a Cursor '${workDir}'`)
        : `cd '${workDir}' && claude --resume ${session.sessionId}`;
    navigator.clipboard.writeText(cmd).then(() => {
      setCopiedId(session.sessionId);
      setTimeout(() => setCopiedId(null), 2000);
    });
  };

  const getSessionNavKey = (session: any): string => {
    if (activeTool === "codex") {
      return encodeURIComponent(session.filePath);
    }
    if (activeTool === "copilot") {
      return session.sessionId;
    }
    if (activeTool === "cursor" || activeTool === "cursor-cli") {
      return session.sessionId;
    }
    return session.sessionId;
  };

  // Check if sessions data is grouped (for OpenCode)
  const isGroupedData = sessions.length > 0 && sessions[0].rootSession !== undefined;

  return (
    <div className="p-6">
      {/* Header */}
      <div className="flex items-center gap-3 mb-6">
        <button
          onClick={() => navigate(`/${activeTool}/projects`)}
          className="p-1 rounded hover:bg-accent transition-colors"
        >
          <ArrowLeft className="w-5 h-5" />
        </button>
        <div>
          <h1 className="text-2xl font-bold">
            {project?.shortName || projectKey}
          </h1>
          {project && (
            <p className="text-sm text-muted-foreground mt-0.5">
              {activeTool === "codex"
                ? project.cwd
                : activeTool === "copilot"
                ? project.cwd
                : activeTool === "cursor" || activeTool === "cursor-cli"
                ? project.cwd
                : project.displayPath}
            </p>
          )}
        </div>
      </div>

      {/* Sessions list */}
      {sessionsLoading ? (
        <div className="text-muted-foreground">加载会话列表...</div>
      ) : sessions.length === 0 ? (
        <div className="text-muted-foreground">此项目没有会话记录。</div>
      ) : isGroupedData ? (
        // OpenCode grouped sessions
        <OpencodeSessionList
          groups={sessions as OpencodeSessionGroup[]}
          projectKey={projectKey!}
          activeTool={activeTool}
        />
      ) : (
        // Regular flat sessions list (Claude, Codex)
        <div className="space-y-2">
          {sessions.map((session) => (
            <div
              key={session.sessionId}
              onClick={() =>
                navigate(
                  `/${activeTool}/projects/${encodeURIComponent(projectKey!)}/session/${getSessionNavKey(session)}`
                )
              }
              className="bg-card border border-border rounded-lg p-4 hover:border-primary/50 hover:bg-accent/30 transition-all cursor-pointer group"
            >
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-foreground line-clamp-2">
                    {session.firstPrompt || "（无标题）"}
                  </p>
                  <div className="flex items-center gap-4 mt-2 text-xs text-muted-foreground flex-wrap">
                    {session.messageCount != null && (
                      <span className="flex items-center gap-1">
                        <MessageSquare className="w-3 h-3" />
                        {session.messageCount} 条消息
                      </span>
                    )}
                    {session.gitBranch && (
                      <span className="flex items-center gap-1">
                        <GitBranch className="w-3 h-3" />
                        {session.gitBranch}
                      </span>
                    )}
                    {activeTool === "copilot" && session.summary && (
                      <span className="text-muted-foreground/80 italic truncate max-w-xs">
                        {session.summary}
                      </span>
                    )}
                    {activeTool === "copilot" && session.branch && (
                      <span className="flex items-center gap-1">
                        <GitBranch className="w-3 h-3" />
                        {session.branch}
                      </span>
                    )}
                    {activeTool === "copilot"
                      ? (session.updatedAt || session.createdAt) && (
                          <span className="flex items-center gap-1">
                            <Clock className="w-3 h-3" />
                            {formatDistanceToNow(
                              new Date(session.updatedAt || session.createdAt),
                              { addSuffix: true, locale: zhCN }
                            )}
                          </span>
                        )
                      : session.modified && (
                          <span className="flex items-center gap-1">
                            <Clock className="w-3 h-3" />
                            {formatDistanceToNow(
                              new Date(session.modified),
                              { addSuffix: true, locale: zhCN }
                            )}
                          </span>
                        )}
                    {activeTool !== "copilot" && session.created && (
                      <span className="text-muted-foreground/60">
                        创建于{" "}
                        {format(new Date(session.created), "yyyy-MM-dd HH:mm")}
                      </span>
                    )}
                    {session.models && session.models.length > 0 && (
                      <span className="px-1.5 py-0.5 bg-primary/10 text-primary rounded text-xs font-mono">
                        {session.models.join(" / ")}
                      </span>
                    )}
                  </div>
                </div>
                <div
                  className="shrink-0 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity"
                  onClick={(e) => e.stopPropagation()}
                >
                  <button
                    onClick={(e) => handleResume(e, session)}
                    className="px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded-md hover:bg-primary/90 flex items-center gap-1"
                    title={(activeTool === "cursor" || activeTool === "cursor-cli") ? "在 Cursor 中打开此 workspace" : "在终端中恢复此会话"}
                  >
                    <Play className="w-3 h-3" />
                    {(activeTool === "cursor" || activeTool === "cursor-cli") ? "打开" : "Resume"}
                  </button>
                  <button
                    onClick={(e) => handleCopy(e, session)}
                    className="px-2 py-1.5 text-xs bg-secondary text-secondary-foreground rounded-md hover:bg-secondary/80 flex items-center gap-1"
                    title="复制恢复命令到剪贴板"
                  >
                    {copiedId === session.sessionId ? (
                      <Check className="w-3 h-3 text-green-500" />
                    ) : (
                      <Copy className="w-3 h-3" />
                    )}
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
