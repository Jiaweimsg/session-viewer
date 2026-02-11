import { useEffect, useRef, useCallback, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import { ArrowLeft, Play, Loader2, ArrowDown, Copy, Check } from "lucide-react";
import { MessageThread } from "./MessageThread";
import { resumeSession } from "../../services/tauriApi";

export function MessagesPage() {
  const { projectKey, sessionKey } = useParams<{
    tool: string;
    projectKey: string;
    sessionKey: string;
  }>();
  const navigate = useNavigate();
  const {
    activeTool,
    messages,
    messagesLoading,
    messagesHasMore,
    messagesTotal,
    selectSession,
    loadMoreMessages,
    loadAllMessages,
    sessions,
    projects,
  } = useAppStore();

  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [showScrollBottom, setShowScrollBottom] = useState(false);
  const [idle, setIdle] = useState(false);
  const idleTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const notAtBottom = useRef(false);
  const [copied, setCopied] = useState(false);

  // Hide button after 5s of no scroll activity
  const resetIdleTimer = useCallback(() => {
    setIdle(false);
    clearTimeout(idleTimer.current);
    idleTimer.current = setTimeout(() => setIdle(true), 5000);
  }, []);

  const session =
    activeTool === "codex"
      ? sessions.find((s) => encodeURIComponent(s.filePath) === sessionKey)
      : sessions.find((s) => s.sessionId === sessionKey);

  const project =
    activeTool === "codex"
      ? projects.find((p) => encodeURIComponent(p.cwd) === projectKey)
      : projects.find((p) => p.encodedName === projectKey);

  useEffect(() => {
    if (sessionKey) {
      if (activeTool === "claude") {
        selectSession(sessionKey, projectKey);
      } else {
        selectSession(decodeURIComponent(sessionKey));
      }
    }
  }, [sessionKey, projectKey]);

  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    notAtBottom.current = scrollHeight - scrollTop - clientHeight > 400;
    setShowScrollBottom(notAtBottom.current);
    resetIdleTimer();
    if (!messagesLoading && messagesHasMore && scrollHeight - scrollTop - clientHeight < 200) {
      loadMoreMessages();
    }
  }, [messagesLoading, messagesHasMore, loadMoreMessages, resetIdleTimer]);

  const scrollToBottom = async () => {
    if (messagesHasMore) {
      await loadAllMessages();
    }
    // Wait for DOM update then scroll
    requestAnimationFrame(() => {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    });
  };

  const handleResume = async () => {
    const sid = session?.sessionId || sessionKey;
    if (!sid) return;
    const workDir =
      activeTool === "codex"
        ? session?.cwd
        : session?.projectPath || project?.displayPath;
    if (!workDir) return;
    try {
      await resumeSession(activeTool, sid, workDir, session?.fullPath || undefined);
    } catch (err) {
      console.error("Failed to resume session:", err);
    }
  };

  const handleCopy = () => {
    const sid = session?.sessionId || sessionKey;
    if (!sid) return;
    const workDir =
      activeTool === "codex"
        ? session?.cwd
        : session?.projectPath || project?.displayPath;
    if (!workDir) return;
    const cmd =
      activeTool === "codex"
        ? `cd '${workDir}' && codex resume ${sid}`
        : `cd '${workDir}' && claude --resume ${sid}`;
    navigator.clipboard.writeText(cmd).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="shrink-0 border-b border-border bg-card px-6 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3 min-w-0">
          <button
            onClick={() => navigate(`/${activeTool}/projects/${encodeURIComponent(projectKey!)}`)}
            className="p-1 rounded hover:bg-accent transition-colors shrink-0"
          >
            <ArrowLeft className="w-5 h-5" />
          </button>
          <div className="min-w-0">
            <p className="text-sm font-medium truncate">
              {session?.firstPrompt || sessionKey}
            </p>
            <p className="text-xs text-muted-foreground">
              {messagesTotal} 条消息
              {session?.gitBranch && ` · ${session.gitBranch}`}
            </p>
          </div>
        </div>
        <div className="shrink-0 flex gap-1">
          <button
            onClick={handleResume}
            className="px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded-md hover:bg-primary/90 flex items-center gap-1"
          >
            <Play className="w-3 h-3" />
            Resume
          </button>
          <button
            onClick={handleCopy}
            className="px-2 py-1.5 text-xs bg-secondary text-secondary-foreground rounded-md hover:bg-secondary/80 flex items-center gap-1"
            title="复制恢复命令到剪贴板"
          >
            {copied ? (
              <Check className="w-3 h-3 text-green-500" />
            ) : (
              <Copy className="w-3 h-3" />
            )}
          </button>
        </div>
      </div>

      {/* Messages */}
      <div className="relative flex-1">
        <div
          ref={containerRef}
          onScroll={handleScroll}
          className="absolute inset-0 overflow-y-auto"
        >
        {messagesLoading && messages.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-muted-foreground">
            <Loader2 className="w-5 h-5 animate-spin mr-2" />
            加载消息中...
          </div>
        ) : (
          <>
            <MessageThread messages={messages} />
            {messagesLoading && messages.length > 0 && (
              <div className="flex items-center justify-center py-4 text-muted-foreground">
                <Loader2 className="w-4 h-4 animate-spin mr-2" />
                加载更多...
              </div>
            )}
            {!messagesHasMore && messages.length > 0 && (
              <div className="text-center py-4 text-xs text-muted-foreground">
                — 会话结束 —
              </div>
            )}
          </>
        )}
        <div ref={bottomRef} />
        </div>
        {showScrollBottom && !idle && (
          <button
            onClick={scrollToBottom}
            className="absolute right-4 bottom-4 p-3 rounded-full bg-primary text-primary-foreground shadow-lg hover:bg-primary/90 transition-opacity"
            title="滚动到底部"
          >
            <ArrowDown className="w-5 h-5" />
          </button>
        )}
      </div>
    </div>
  );
}
