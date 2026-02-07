// Tool type discriminator
export type ToolType = "claude" | "codex";

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
  totalTokens: number;
  tokensByModel: Record<string, number>;
  dailyTokens: { date: string; tokens: number }[];
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
