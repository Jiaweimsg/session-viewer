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

export async function getAdvancedStats(tool: ToolType): Promise<any> {
  return invoke("get_advanced_stats", { tool });
}

export async function reportUsage(serverUrl: string): Promise<{ ok?: boolean; received?: number; error?: string }> {
  return invoke("report_usage", { serverUrl });
}

export async function resumeSession(
  tool: ToolType,
  sessionId: string,
  workDir: string,
  filePath?: string
): Promise<void> {
  return invoke<void>("resume_session", { tool, sessionId, workDir, filePath });
}

export interface UploadBlocklist {
  cwd_prefixes: string[];
}

export async function getUploadBlocklist(): Promise<UploadBlocklist> {
  return invoke<UploadBlocklist>("get_upload_blocklist");
}

export async function setUploadBlocklist(blocklist: UploadBlocklist): Promise<void> {
  return invoke<void>("set_upload_blocklist", { blocklist });
}

export interface IdentityOverride {
  user_name?: string | null;
  user_email?: string | null;
}

export interface IdentityView {
  effective_email: string;
  effective_name: string;
  override_email: string | null;
  override_name: string | null;
  git_email: string | null;
  git_name: string | null;
  os_user: string;
  hostname: string;
}

export async function getIdentityView(): Promise<IdentityView> {
  return invoke<IdentityView>("get_identity_view");
}

export async function getIdentityOverride(): Promise<IdentityOverride> {
  return invoke<IdentityOverride>("get_identity_override");
}

export async function setIdentityOverride(identity: IdentityOverride): Promise<void> {
  return invoke<void>("set_identity_override", { identity });
}
