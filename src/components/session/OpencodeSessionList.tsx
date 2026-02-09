import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import type { OpencodeSessionGroup } from "../../types";
import {
    MessageSquare,
    Clock,
    Play,
    Copy,
    Check,
    ChevronRight,
    ChevronDown,
    Layers,
} from "lucide-react";
import { formatDistanceToNow, format } from "date-fns";
import { zhCN } from "date-fns/locale";
import { resumeSession } from "../../services/tauriApi";

interface Props {
    groups: OpencodeSessionGroup[];
    projectKey: string;
    activeTool: string;
}

export function OpencodeSessionList({ groups, projectKey, activeTool }: Props) {
    const navigate = useNavigate();
    const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
    const [copiedId, setCopiedId] = useState<string | null>(null);

    const toggleGroup = (sessionId: string) => {
        setExpandedGroups((prev) => {
            const next = new Set(prev);
            if (next.has(sessionId)) {
                next.delete(sessionId);
            } else {
                next.add(sessionId);
            }
            return next;
        });
    };

    const handleResume = async (e: React.MouseEvent, sessionId: string, directory: string) => {
        e.stopPropagation();
        try {
            await resumeSession(activeTool as any, sessionId, directory);
        } catch (err) {
            console.error("Failed to resume session:", err);
        }
    };

    const handleCopy = (e: React.MouseEvent, sessionId: string, directory: string) => {
        e.stopPropagation();
        const cmd = `cd '${directory}' && opencode --session ${sessionId}`;
        navigator.clipboard.writeText(cmd).then(() => {
            setCopiedId(sessionId);
            setTimeout(() => setCopiedId(null), 2000);
        });
    };

    const handleNavigate = (sessionId: string) => {
        navigate(
            `/${activeTool}/projects/${encodeURIComponent(projectKey)}/session/${sessionId}`
        );
    };

    return (
        <div className="space-y-2">
            {groups.map((group) => {
                const { rootSession, subSessions } = group;
                const isExpanded = expandedGroups.has(rootSession.sessionId);
                const hasChildren = subSessions.length > 0;

                return (
                    <div key={rootSession.sessionId} className="space-y-1">
                        {/* Root Session */}
                        <div
                            onClick={() => {
                                if (hasChildren) {
                                    toggleGroup(rootSession.sessionId);
                                } else {
                                    handleNavigate(rootSession.sessionId);
                                }
                            }}
                            className="bg-card border border-border rounded-lg p-4 hover:border-primary/50 hover:bg-accent/30 transition-all cursor-pointer group"
                        >
                            <div className="flex items-start justify-between gap-4">
                                <div className="min-w-0 flex-1 flex items-start gap-2">
                                    {/* Expand/Collapse Icon */}
                                    {hasChildren && (
                                        <div className="shrink-0 mt-1">
                                            {isExpanded ? (
                                                <ChevronDown className="w-4 h-4 text-muted-foreground" />
                                            ) : (
                                                <ChevronRight className="w-4 h-4 text-muted-foreground" />
                                            )}
                                        </div>
                                    )}

                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-2 mb-1">
                                            <p className="text-sm font-medium text-foreground line-clamp-2">
                                                {rootSession.title || rootSession.firstPrompt || "（无标题）"}
                                            </p>
                                            {hasChildren && (
                                                <span className="shrink-0 px-2 py-0.5 bg-primary/10 text-primary rounded text-xs font-medium flex items-center gap-1">
                                                    <Layers className="w-3 h-3" />
                                                    {subSessions.length}
                                                </span>
                                            )}
                                        </div>

                                        <div className="flex items-center gap-4 mt-2 text-xs text-muted-foreground flex-wrap">
                                            <span className="flex items-center gap-1">
                                                <MessageSquare className="w-3 h-3" />
                                                {rootSession.messageCount} 条消息
                                            </span>
                                            {rootSession.modified && (
                                                <span className="flex items-center gap-1">
                                                    <Clock className="w-3 h-3" />
                                                    {formatDistanceToNow(new Date(rootSession.modified), {
                                                        addSuffix: true,
                                                        locale: zhCN,
                                                    })}
                                                </span>
                                            )}
                                            {rootSession.slug && (
                                                <span className="text-muted-foreground/60 font-mono">
                                                    {rootSession.slug}
                                                </span>
                                            )}
                                        </div>
                                    </div>
                                </div>

                                {/* Action Buttons */}
                                <div
                                    className="shrink-0 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity"
                                    onClick={(e) => e.stopPropagation()}
                                >
                                    <button
                                        onClick={(e) =>
                                            handleResume(e, rootSession.sessionId, rootSession.directory)
                                        }
                                        className="px-3 py-1.5 text-xs bg-primary text-primary-foreground rounded-md hover:bg-primary/90 flex items-center gap-1"
                                        title="在终端中恢复此会话"
                                    >
                                        <Play className="w-3 h-3" />
                                        Resume
                                    </button>
                                    <button
                                        onClick={(e) =>
                                            handleCopy(e, rootSession.sessionId, rootSession.directory)
                                        }
                                        className="px-2 py-1.5 text-xs bg-secondary text-secondary-foreground rounded-md hover:bg-secondary/80 flex items-center gap-1"
                                        title="复制恢复命令到剪贴板"
                                    >
                                        {copiedId === rootSession.sessionId ? (
                                            <Check className="w-3 h-3 text-green-500" />
                                        ) : (
                                            <Copy className="w-3 h-3" />
                                        )}
                                    </button>
                                </div>
                            </div>
                        </div>

                        {/* Sub Sessions (Collapsed/Expanded) */}
                        {hasChildren && isExpanded && (
                            <div className="ml-6 pl-4 border-l-2 border-primary/20 space-y-1">
                                {subSessions.map((subSession) => (
                                    <div
                                        key={subSession.sessionId}
                                        onClick={() => handleNavigate(subSession.sessionId)}
                                        className="bg-card/50 border border-border/50 rounded-lg p-3 hover:border-primary/30 hover:bg-accent/20 transition-all cursor-pointer group"
                                    >
                                        <div className="flex items-start justify-between gap-4">
                                            <div className="min-w-0 flex-1">
                                                <div className="flex items-center gap-2 mb-1">
                                                    <span className="shrink-0 px-1.5 py-0.5 bg-orange-500/10 text-orange-500 rounded text-[10px] font-medium">
                                                        SUBAGENT
                                                    </span>
                                                    <p className="text-xs font-medium text-foreground/90 line-clamp-1">
                                                        {subSession.title || subSession.firstPrompt || "（子会话）"}
                                                    </p>
                                                </div>

                                                <div className="flex items-center gap-3 text-[11px] text-muted-foreground/80 flex-wrap">
                                                    <span className="flex items-center gap-1">
                                                        <MessageSquare className="w-2.5 h-2.5" />
                                                        {subSession.messageCount}
                                                    </span>
                                                    {subSession.created && (
                                                        <span>
                                                            {format(new Date(subSession.created), "HH:mm")}
                                                        </span>
                                                    )}
                                                </div>
                                            </div>

                                            {/* Sub Session Actions */}
                                            <div
                                                className="shrink-0 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity"
                                                onClick={(e) => e.stopPropagation()}
                                            >
                                                <button
                                                    onClick={(e) =>
                                                        handleResume(e, subSession.sessionId, subSession.directory)
                                                    }
                                                    className="px-2 py-1 text-[10px] bg-primary/80 text-primary-foreground rounded hover:bg-primary flex items-center gap-1"
                                                >
                                                    <Play className="w-2.5 h-2.5" />
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                );
            })}
        </div>
    );
}
