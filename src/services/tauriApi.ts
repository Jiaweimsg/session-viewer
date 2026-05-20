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

export async function reportUsage(serverUrl: string): Promise<{ ok?: boolean; received?: number; error?: string; ranking?: RankingPayload | null }> {
  return invoke("report_usage", { serverUrl });
}

export interface RankingEntry {
  rank: number;
  medal: "gold" | "silver" | "bronze" | null;
  email: string;
  name: string | null;
  remark: string | null;
  client_version: string | null;
  total_tokens: number;
  estimated_cost: number;
  message_count?: number | null;
}

export interface RankingPayload {
  date: string;          // YYYY-MM-DD (server-side CST)
  top3: RankingEntry[];
  your_rank: number | null;
  your_cost: number;
  /** Cost of the user one rank above the reporter. Null when reporter is
   *  rank 1 or has no spend today. Lets the UI show "差 $X.XX 追上 #N-1"
   *  even when the user is outside top 3. */
  your_next_cost: number | null;
  /** Display name (remark || name || email) of the user one rank above —
   *  so the banner can show "追上 alice" instead of "追上 #N-1" even when
   *  that user isn't in top3. Null mirrors your_next_cost. Servers
   *  <= 0.4.2 omit this. */
  your_next_name: string | null;
  /** Cost of the user one rank below — the chaser. Drives the "被追"
   *  chip. Null when reporter is last or has no spend today. */
  your_chaser_cost: number | null;
  /** Display name of the chaser, same priority as your_next_name. */
  your_chaser_name: string | null;
  total_ranked: number;
}

/** Latest daily leaderboard piggybacked on /api/report responses. Null until
 *  the first report cycle completes (auto: ~30s after launch). */
export async function getLatestRanking(): Promise<RankingPayload | null> {
  return invoke<RankingPayload | null>("get_latest_ranking");
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

export async function resetConversationState(): Promise<void> {
  return invoke<void>("reset_conversation_state");
}
