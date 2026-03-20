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
  ChevronDown,
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
    } else if (activeTool === "opencode") {
      return project.id;
    } else if (activeTool === "copilot") {
      return encodeURIComponent(project.cwd);
    } else if (activeTool === "cursor") {
      return project.encodedName;
    }
    return project.encodedName;
  };

  const getProjectTitle = (project: any): string => {
    if (activeTool === "codex") {
      return project.cwd;
    } else if (activeTool === "opencode") {
      return project.worktree;
    } else if (activeTool === "copilot") {
      return project.cwd;
    } else if (activeTool === "cursor") {
      return project.displayPath;
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
        <div className="relative mt-3">
          <select
            value={activeTool}
            onChange={(e) => handleToolSwitch(e.target.value as ToolType)}
            className="w-full appearance-none bg-muted text-foreground text-sm font-medium rounded-md px-3 py-2 pr-8 border border-border focus:outline-none focus:ring-2 focus:ring-ring cursor-pointer"
          >
            <option value="claude">Claude Code</option>
            <option value="codex">Codex</option>
            <option value="opencode">OpenCode</option>
            <option value="copilot">Copilot</option>
            <option value="cursor">Cursor</option>
          </select>
          <ChevronDown className="absolute right-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground pointer-events-none" />
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto p-2">
        {/* Quick links */}
        <div className="mb-4">
          <button
            onClick={() => navigate(`/${activeTool}/search`)}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${isActive(`/${activeTool}/search`)
                ? "bg-accent text-accent-foreground"
                : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
              }`}
          >
            <Search className="w-4 h-4" />
            全局搜索
          </button>
          <button
            onClick={() => navigate(`/${activeTool}/stats`)}
            className={`w-full flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${isActive(`/${activeTool}/stats`)
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
                    className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-md text-sm transition-colors group ${isProjectActive(projectKey)
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
