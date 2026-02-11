import { invoke } from "@tauri-apps/api/core";
import type { ToolType, PaginatedMessages } from "../types";

export async function getProjects(tool: ToolType): Promise<any[]> {
  return invoke("get_projects", { tool });
}

export async function getSessions(
  tool: ToolType,
  projectKey: string
): Promise<any[]> {
  return invoke("get_sessions", { tool, projectKey });
}

export async function getSessionsGrouped(
  tool: ToolType,
  projectKey: string
): Promise<any[]> {
  return invoke("get_sessions_grouped", { tool, projectKey });
}

export async function getMessages(
  tool: ToolType,
  sessionKey: string,
  projectKey: string | null,
  page: number = 0,
  pageSize: number = 50
): Promise<PaginatedMessages> {
  return invoke<PaginatedMessages>("get_messages", {
    tool,
    sessionKey,
    projectKey,
    page,
    pageSize,
  });
}

export async function globalSearch(
  tool: ToolType,
  query: string,
  maxResults: number = 50
): Promise<any[]> {
  return invoke("global_search", { tool, query, maxResults });
}

export async function getStats(tool: ToolType): Promise<any> {
  return invoke("get_stats", { tool });
}

export async function getTokenSummary(tool: ToolType): Promise<any> {
  return invoke("get_token_summary", { tool });
}

export async function resumeSession(
  tool: ToolType,
  sessionId: string,
  workDir: string,
  filePath?: string
): Promise<void> {
  return invoke<void>("resume_session", { tool, sessionId, workDir, filePath });
}
