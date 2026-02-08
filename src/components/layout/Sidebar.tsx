import { useEffect, useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import type { ToolType } from "../../types";
import {
  FolderOpen,
  Search,
  BarChart3,
  ChevronRight,
  Palette,
  RefreshCw,
} from "lucide-react";
import { ThemePicker } from "./ThemePicker";

export function Sidebar() {
  const navigate = useNavigate();
  const location = useLocation();
  const { activeTool, setActiveTool, projects, loadProjects, projectsLoading } =
    useAppStore();
  const [showThemePicker, setShowThemePicker] = useState(false);

  useEffect(() => {
    loadProjects();
  }, [activeTool]);

  const isActive = (path: string) => location.pathname === path;
  const isProjectActive = (projectKey: string) =>
    location.pathname.startsWith(`/${activeTool}/projects/${projectKey}`);

  const handleToolSwitch = (tool: ToolType) => {
    setActiveTool(tool);
    navigate(`/${tool}/projects`);
  };

  const getProjectKey = (project: any): string => {
    if (activeTool === "codex") {
      return encodeURIComponent(project.cwd);
    }
    return project.encodedName;
  };

  const getProjectTitle = (project: any): string => {
    if (activeTool === "codex") {
      return project.cwd;
    }
    return project.displayPath;
  };

  return (
    <aside className="w-64 h-full border-r border-border bg-card flex flex-col shrink-0">
      {/* Header */}
      <div className="p-4 border-b border-border">
        <h1 className="text-sm font-semibold text-foreground flex items-center gap-2">
          <img src="/logo.png" alt="Session Viewer" className="w-5 h-5 rounded" />
          Session Viewer
        </h1>
        {/* Tool switcher */}
        <div className="flex mt-3 gap-1">
          <button
            onClick={() => handleToolSwitch("claude")}
            className={`flex-1 px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${
              activeTool === "claude"
                ? "bg-primary text-primary-foreground"
                : "bg-muted text-muted-foreground hover:bg-accent"
            }`}
          >
            Claude Code
          </button>
          <button
            onClick={() => handleToolSwitch("codex")}
            className={`flex-1 px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${
              activeTool === "codex"
                ? "bg-primary text-primary-foreground"
                : "bg-muted text-muted-foreground hover:bg-accent"
            }`}
          >
            Codex
          </button>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto p-2">
        {/* Quick links */}
        <div className="mb-4">
          <button
            onClick={() => navigate(`/${activeTool}/search`)}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive(`/${activeTool}/search`)
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
            }`}
          >
            <Search className="w-4 h-4" />
            全局搜索
          </button>
          <button
            onClick={() => navigate(`/${activeTool}/stats`)}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
              isActive(`/${activeTool}/stats`)
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
            }`}
          >
            <BarChart3 className="w-4 h-4" />
            使用统计
          </button>
        </div>

        {/* Projects list */}
        <div>
          <div className="flex items-center justify-between px-3 py-1">
            <h2 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              项目 ({projects.length})
            </h2>
            <button
              onClick={() => loadProjects()}
              disabled={projectsLoading}
              className="p-0.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50"
              title="刷新项目列表"
            >
              <RefreshCw className={`w-3 h-3 ${projectsLoading ? "animate-spin" : ""}`} />
            </button>
          </div>
          {projectsLoading ? (
            <div className="px-3 py-2 text-sm text-muted-foreground">
              加载中...
            </div>
          ) : (
            <div className="mt-1 space-y-0.5">
              {projects.map((project) => {
                const projectKey = getProjectKey(project);
                return (
                  <button
                    key={projectKey}
                    onClick={() =>
                      navigate(`/${activeTool}/projects/${projectKey}`)
                    }
                    className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-md text-sm transition-colors group ${
                      isProjectActive(projectKey)
                        ? "bg-accent text-accent-foreground"
                        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                    }`}
                    title={getProjectTitle(project)}
                  >
                    <FolderOpen className="w-3.5 h-3.5 shrink-0" />
                    <span className="truncate flex-1 text-left">
                      {project.shortName}
                    </span>
                    <span className="text-xs text-muted-foreground shrink-0">
                      {project.sessionCount}
                    </span>
                    <ChevronRight className="w-3 h-3 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity" />
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </nav>

      {/* Footer */}
      <div className="relative p-3 border-t border-border text-xs text-muted-foreground flex items-center justify-between">
        <span>{projects.length} 个项目</span>
        <button
          onClick={() => setShowThemePicker(!showThemePicker)}
          className="p-1 rounded hover:bg-accent transition-colors"
          title="切换主题"
        >
          <Palette className="w-4 h-4" />
        </button>
        {showThemePicker && (
          <ThemePicker onClose={() => setShowThemePicker(false)} />
        )}
      </div>
    </aside>
  );
}
