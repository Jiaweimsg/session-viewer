// Tool type discriminator
export type ToolType = "claude" | "codex" | "opencode" | "copilot" | "cursor" | "cursor-cli";

// ============ Projects ============

export interface ClaudeProject {
  encodedName: string;
  displayPath: string;
  shortName: string;
  sessionCount: number;
  lastModified: string | null;
}

export interface CodexProject {
  cwd: string;
  shortName: string;
  sessionCount: number;
  lastModified: string | null;
  modelProvider: string | null;
}

export interface OpencodeProject {
  id: string;
  worktree: string;
  shortName: string;
  sessionCount: number;
  lastModified: string | null;
}

// ============ Sessions ============

export interface ClaudeSession {
  sessionId: string;
  fullPath: string | null;
  fileMtime: number | null;
  firstPrompt: string | null;
  messageCount: number | null;
  created: string | null;
  modified: string | null;
  gitBranch: string | null;
  projectPath: string | null;
  isSidechain: boolean | null;
}

export interface CodexSession {
  sessionId: string;
  cwd: string;
  shortName: string;
  model: string | null;
  modelProvider: string | null;
  cliVersion: string | null;
  firstPrompt: string | null;
  messageCount: number;
  created: string | null;
  modified: string | null;
  gitBranch: string | null;
  filePath: string;
}

export interface OpencodeSession {
  sessionId: string;
  projectId: string;
  directory: string;
  shortName: string;
  title: string | null;
  slug: string | null;
  firstPrompt: string | null;
  messageCount: number;
  created: string | null;
  modified: string | null;
  gitBranch: string | null;
  parentId: string | null;  // 添加 parentId 字段
}

export interface OpencodeSessionGroup {
  rootSession: OpencodeSession;
  subSessions: OpencodeSession[];
}

// ============ Messages (unified) ============

export type DisplayContentBlock =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string }
  | { type: "tool_use"; id: string; name: string; input: string }
  | {
    type: "tool_result";
    toolUseId: string;
    content: string;
    isError: boolean;
  }
  | { type: "reasoning"; text: string }
  | {
    type: "function_call";
    name: string;
    arguments: string;
    callId: string;
  }
  | { type: "function_call_output"; callId: string; output: string };

export interface DisplayMessage {
  uuid: string | null;
  role: string;
  timestamp: string | null;
  content: DisplayContentBlock[];
}

export interface PaginatedMessages {
  messages: DisplayMessage[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
}

// ============ Stats ============

// Claude stats
export interface DailyActivity {
  date: string;
  messageCount: number;
  sessionCount: number;
  toolCallCount: number;
}

export interface DailyModelTokens {
  date: string;
  tokensByModel: Record<string, number>;
}

export interface StatsCache {
  version: number | null;
  lastComputedDate: string | null;
  dailyActivity: DailyActivity[];
  dailyModelTokens: DailyModelTokens[];
}

export interface ClaudeTokenSummary {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  tokensByModel: Record<string, number>;
  dailyTokens: { date: string; inputTokens: number; outputTokens: number; totalTokens: number }[];
}

// Claude advanced stats
export interface AdvancedStats {
  projectTokenRanking: ProjectTokenEntry[];
  toolCallRanking: ToolCallEntry[];
  efficiency: SessionEfficiency;
}

export interface ProjectTokenEntry {
  projectName: string;
  totalTokens: number;
  inputTokens: number;
  outputTokens: number;
}

export interface ToolCallEntry {
  toolName: string;
  callCount: number;
}

export interface SessionEfficiency {
  avgMessagesPerSession: number;
  avgTokensPerSession: number;
  totalSessions: number;
  totalMessages: number;
  distribution: EfficiencyBucket[];
}

export interface EfficiencyBucket {
  label: string;
  count: number;
}

// Codex stats
export interface CodexTokenSummary {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  tokensByModel: Record<string, number>;
  dailyTokens: CodexDailyTokenEntry[];
  sessionCount: number;
  messageCount: number;
}

export interface CodexDailyTokenEntry {
  date: string;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
}

// ============ Search ============

export interface ClaudeSearchResult {
  encodedName: string;
  projectName: string;
  sessionId: string;
  firstPrompt: string | null;
  matchedText: string;
  role: string;
  timestamp: string | null;
}

export interface CodexSearchResult {
  cwd: string;
  shortName: string;
  sessionId: string;
  firstPrompt: string | null;
  matchedText: string;
  role: string;
  timestamp: string | null;
  filePath: string;
}

export interface OpencodeSearchResult {
  projectId: string;
  sessionId: string;
  firstPrompt: string | null;
  matchedText: string;
  role: string;
  timestamp: string | null;
  messageId: string;
}

// ============ Copilot ============

export interface CopilotProject {
  cwd: string;
  shortName: string;
  sessionCount: number;
  lastModified: string | null;
}

export interface CopilotSession {
  sessionId: string;
  cwd: string;
  gitRoot: string | null;
  branch: string | null;
  summary: string | null;
  createdAt: string;
  updatedAt: string | null;
  messageCount: number;
  firstPrompt: string | null;
}

export interface CopilotStats {
  totalSessions: number;
  totalProjects: number;
}

// ============ Cursor ============

export interface CursorProject {
  cwd: string;
  shortName: string;
  sessionCount: number;
  lastModified: string | null;
  messageCount: number;
}

export interface CursorSession {
  sessionId: string;
  name: string | null;
  mode: string | null;
  firstPrompt: string | null;
  messageCount: number;
  created: string | null;
  modified: string | null;
  isArchived: boolean;
}

export interface CursorSearchResult {
  sessionId: string;
  projectName: string | null;
  sessionName: string | null;
  matchedText: string;
  role: string;
  timestamp: string | null;
}

export interface CursorDailyActivity {
  date: string;
  messageCount: number;
  sessionCount: number;
}

export interface CursorDailyTokenEntry {
  date: string;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
}

export interface CursorModeEntry {
  mode: string;
  count: number;
}

export interface CursorProjectTokenEntry {
  projectName: string;
  totalTokens: number;
  inputTokens: number;
  outputTokens: number;
  sessionCount: number;
  messageCount: number;
}

export interface CursorSessionEfficiency {
  avgMessagesPerSession: number;
  avgTokensPerSession: number;
  totalSessions: number;
  totalMessages: number;
  distribution: { label: string; count: number }[];
}

export interface CursorModelUsageEntry {
  model: string;
  requestCount: number;
}

export interface CursorStats {
  totalSessions: number;
  totalProjects: number;
  totalMessages: number;
  totalRequests: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  dailyActivity: CursorDailyActivity[];
  dailyTokens: CursorDailyTokenEntry[];
  modeDistribution: CursorModeEntry[];
  modelUsage: CursorModelUsageEntry[];
  projectRanking: CursorProjectTokenEntry[];
  efficiency: CursorSessionEfficiency;
  activeSessions: number;
  archivedSessions: number;
}
