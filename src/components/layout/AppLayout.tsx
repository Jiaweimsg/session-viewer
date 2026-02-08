import { useEffect } from "react";
import { Outlet, useParams } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { useAppStore } from "../../stores/appStore";
import { useTheme } from "../../hooks/useTheme";
import type { ToolType } from "../../types";

export function AppLayout() {
  const { tool } = useParams<{ tool: string }>();
  const { activeTool, setActiveTool } = useAppStore();
  useTheme(); // Initialize theme on app mount

  // Sync URL tool param to store
  useEffect(() => {
    if (tool && (tool === "claude" || tool === "codex") && tool !== activeTool) {
      setActiveTool(tool as ToolType);
    }
  }, [tool]);

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar />
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
