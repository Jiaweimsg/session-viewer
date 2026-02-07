import { useEffect } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import {
  ArrowLeft,
  MessageSquare,
  Clock,
  GitBranch,
  Play,
} from "lucide-react";
import { formatDistanceToNow, format } from "date-fns";
import { zhCN } from "date-fns/locale";
import { resumeSession } from "../../services/tauriApi";

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

  const project =
    activeTool === "codex"
      ? projects.find((p) => encodeURIComponent(p.cwd) === projectKey)
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
        : session.projectPath || project?.displayPath || null;
    if (!workDir) return;
    try {
      await resumeSession(activeTool, session.sessionId, workDir);
    } catch (err) {
      console.error("Failed to resume session:", err);
    }
  };

  const getSessionNavKey = (session: any): string => {
    if (activeTool === "codex") {
      return encodeURIComponent(session.filePath);
    }
    return session.sessionId;
  };

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
              {activeTool === "codex" ? project.cwd : project.displayPath}
            </p>
          )}
        </div>
      </div>

      {/* Sessions list */}
      {sessionsLoading ? (
        <div className="text-muted-foreground">加载会话列表...</div>
      ) : sessions.length === 0 ? (
        <div className="text-muted-foreground">此项目没有会话记录。</div>
      ) : (
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
                    {activeTool === "codex" && session.modelProvider && (
                      <span className="px-1.5 py-0.5 bg-green-500/10 text-green-500 rounded text-xs font-medium">
                        {session.modelProvider}
                      </span>
                    )}
                    {activeTool === "codex" && session.model && (
                      <span className="text-muted-foreground/60 font-mono">
                        {session.model}
                      </span>
                    )}
                    {session.modified && (
                      <span className="flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        {formatDistanceToNow(
                          new Date(session.modified),
                          { addSuffix: true, locale: zhCN }
                        )}
                      </span>
                    )}
                    {session.created && (
                      <span className="text-muted-foreground/60">
                        创建于{" "}
                        {format(new Date(session.created), "yyyy-MM-dd HH:mm")}
                      </span>
                    )}
                  </div>
                </div>
                <button
                  onClick={(e) => handleResume(e, session)}
                  className="shrink-0 px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded-md opacity-0 group-hover:opacity-100 transition-opacity hover:bg-primary/90 flex items-center gap-1"
                  title="在终端中恢复此会话"
                >
                  <Play className="w-3 h-3" />
                  Resume
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
