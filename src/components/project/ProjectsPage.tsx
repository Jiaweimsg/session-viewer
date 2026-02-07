import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useAppStore } from "../../stores/appStore";
import { FolderOpen, Clock, Hash } from "lucide-react";
import { formatDistanceToNow } from "date-fns";
import { zhCN } from "date-fns/locale";

export function ProjectsPage() {
  const navigate = useNavigate();
  const { activeTool, projects, loadProjects, projectsLoading } = useAppStore();

  useEffect(() => {
    loadProjects();
  }, [activeTool]);

  const getProjectKey = (project: any): string => {
    if (activeTool === "codex") {
      return encodeURIComponent(project.cwd);
    }
    return project.encodedName;
  };

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-6">所有项目</h1>

      {projectsLoading ? (
        <div className="text-muted-foreground">加载项目列表...</div>
      ) : projects.length === 0 ? (
        <div className="text-muted-foreground">
          未找到任何项目。
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {projects.map((project) => {
            const projectKey = getProjectKey(project);
            return (
              <button
                key={projectKey}
                onClick={() =>
                  navigate(`/${activeTool}/projects/${projectKey}`)
                }
                className="bg-card border border-border rounded-lg p-4 text-left hover:border-primary/50 hover:bg-accent/30 transition-all group"
              >
                <div className="flex items-start gap-3">
                  <FolderOpen className="w-5 h-5 text-primary mt-0.5 shrink-0" />
                  <div className="min-w-0 flex-1">
                    <h3 className="font-medium text-foreground truncate">
                      {project.shortName}
                    </h3>
                    <p
                      className="text-xs text-muted-foreground truncate mt-1"
                      title={activeTool === "codex" ? project.cwd : project.displayPath}
                    >
                      {activeTool === "codex" ? project.cwd : project.displayPath}
                    </p>
                    <div className="flex items-center gap-4 mt-3 text-xs text-muted-foreground flex-wrap">
                      <span className="flex items-center gap-1">
                        <Hash className="w-3 h-3" />
                        {project.sessionCount} 个会话
                      </span>
                      {project.lastModified && (
                        <span className="flex items-center gap-1">
                          <Clock className="w-3 h-3" />
                          {formatDistanceToNow(
                            new Date(project.lastModified),
                            { addSuffix: true, locale: zhCN }
                          )}
                        </span>
                      )}
                      {activeTool === "codex" && project.modelProvider && (
                        <span className="px-1.5 py-0.5 bg-green-500/10 text-green-500 rounded text-xs font-medium">
                          {project.modelProvider}
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
