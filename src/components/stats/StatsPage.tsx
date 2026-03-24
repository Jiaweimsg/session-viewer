import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../../stores/appStore";
import type { StatsCache, ClaudeTokenSummary, CodexTokenSummary, CursorStats as CursorStatsType, AdvancedStats } from "../../types";
import { ChevronLeft, ChevronRight } from "lucide-react";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
  Cell,
} from "recharts";
import {
  MessageSquare,
  Zap,
  Activity,
  Loader2,
  Calendar,
  ArrowDownUp,
  FolderOpen,
  Wrench,
  TrendingUp,
} from "lucide-react";

export function StatsPage() {
  const { activeTool, stats, tokenSummary, advancedStats, statsLoading, loadStats } =
    useAppStore();

  useEffect(() => {
    loadStats();
  }, [activeTool]);

  if (statsLoading) {
    return (
      <div className="flex items-center justify-center h-64 text-muted-foreground">
        <Loader2 className="w-5 h-5 animate-spin mr-2" />
        加载统计数据...
      </div>
    );
  }

  if (!stats && !tokenSummary) {
    return (
      <div className="p-6 text-muted-foreground">
        未找到统计数据。
      </div>
    );
  }

  if (activeTool === "codex" || activeTool === "opencode") {
    return <CodexStats stats={stats as CodexTokenSummary} />;
  }

  if (activeTool === "cursor" || activeTool === "copilot") {
    return <CursorStatsView stats={stats as CursorStatsType} />;
  }

  return (
    <ClaudeStats
      stats={stats as StatsCache}
      tokenSummary={tokenSummary as ClaudeTokenSummary | null}
      advancedStats={advancedStats as AdvancedStats | null}
    />
  );
}

// Format token count for display
function formatTokens(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

// ============ Claude Stats ============

function ClaudeStats({
  stats,
  tokenSummary,
  advancedStats,
}: {
  stats: StatsCache;
  tokenSummary: ClaudeTokenSummary | null;
  advancedStats: AdvancedStats | null;
}) {
  if (!stats) return null;

  const allDates = useMemo(() => {
    const dates = stats.dailyActivity.map((d) => d.date);
    if (tokenSummary) dates.push(...tokenSummary.dailyTokens.map((d) => d.date));
    return dates;
  }, [stats, tokenSummary]);

  const months = useMemo(() => extractMonths(allDates), [allDates]);
  const [selectedMonth, setSelectedMonth] = useState("all");

  const filterByMonth = (date: string) =>
    selectedMonth === "all" || date.startsWith(selectedMonth);

  const filteredActivity = useMemo(
    () => stats.dailyActivity.filter((d) => filterByMonth(d.date)),
    [stats, selectedMonth]
  );

  const filteredTokens = useMemo(
    () => tokenSummary?.dailyTokens.filter((d) => filterByMonth(d.date)) || [],
    [tokenSummary, selectedMonth]
  );

  const totalMessages = filteredActivity.reduce((sum, d) => sum + d.messageCount, 0);
  const totalSessions = filteredActivity.reduce((sum, d) => sum + d.sessionCount, 0);
  const totalToolCalls = filteredActivity.reduce((sum, d) => sum + d.toolCallCount, 0);

  const activityData = filteredActivity.map((d) => ({
    date: d.date.slice(5),
    messages: d.messageCount,
    sessions: d.sessionCount,
    tools: d.toolCallCount,
  }));

  const tokenData = filteredTokens.map((d) => ({
    date: d.date.slice(5),
    input: d.inputTokens,
    output: d.outputTokens,
    total: d.totalTokens,
  }));

  const filteredInputTokens = filteredTokens.reduce((s, d) => s + d.inputTokens, 0);
  const filteredOutputTokens = filteredTokens.reduce((s, d) => s + d.outputTokens, 0);

  const modelBreakdown = tokenSummary && selectedMonth === "all"
    ? Object.entries(tokenSummary.tokensByModel)
      .sort(([, a], [, b]) => b - a)
      .map(([model, tokens]) => ({
        model: model.replace("claude-", ""),
        tokens,
        pct: ((tokens / tokenSummary.totalTokens) * 100).toFixed(1),
      }))
    : [];

  return (
    <div className="p-6 max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold">使用统计</h1>
        <MonthFilter months={months} selected={selectedMonth} onChange={setSelectedMonth} />
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-1 md:grid-cols-5 gap-4 mb-8">
        <StatCard
          icon={<MessageSquare className="w-5 h-5" />}
          label="总消息数"
          value={totalMessages.toLocaleString()}
        />
        <StatCard
          icon={<Calendar className="w-5 h-5" />}
          label="总会话数"
          value={totalSessions.toLocaleString()}
        />
        <StatCard
          icon={<Zap className="w-5 h-5" />}
          label="工具调用"
          value={totalToolCalls.toLocaleString()}
        />
        <StatCard
          icon={<Activity className="w-5 h-5" />}
          label="输入 Token"
          value={formatTokens(filteredInputTokens)}
        />
        <StatCard
          icon={<Activity className="w-5 h-5" />}
          label="输出 Token"
          value={formatTokens(filteredOutputTokens)}
        />
      </div>

      {/* Activity chart */}
      <div className="bg-card border border-border rounded-lg p-4 mb-6">
        <h2 className="text-sm font-medium mb-4">每日活动</h2>
        <ResponsiveContainer width="100%" height={250}>
          <BarChart data={activityData}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis
              dataKey="date"
              tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
            />
            <YAxis
              tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
            />
            <Tooltip
              contentStyle={{
                backgroundColor: "hsl(var(--card))",
                border: "1px solid hsl(var(--border))",
                borderRadius: "6px",
                fontSize: 12,
              }}
            />
            <Bar dataKey="messages" fill="#3b82f6" name="消息" radius={[2, 2, 0, 0]} />
            <Bar dataKey="tools" fill="#f59e0b" name="工具调用" radius={[2, 2, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>

      {/* Token usage chart */}
      {tokenData.length > 0 && (
        <div className="bg-card border border-border rounded-lg p-4 mb-6">
          <h2 className="text-sm font-medium mb-4">每日 Token 用量</h2>
          <ResponsiveContainer width="100%" height={250}>
            <BarChart data={tokenData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis
                dataKey="date"
                tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
              />
              <YAxis
                tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
                tickFormatter={(v) => formatTokens(v)}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: "hsl(var(--card))",
                  border: "1px solid hsl(var(--border))",
                  borderRadius: "6px",
                  fontSize: 12,
                }}
                formatter={(value: number, name: string) => [formatTokens(value), name === "input" ? "输入" : "输出"]}
              />
              <Bar dataKey="input" stackId="tokens" fill="#3b82f6" name="input" radius={[0, 0, 0, 0]} />
              <Bar dataKey="output" stackId="tokens" fill="#f59e0b" name="output" radius={[2, 2, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Model breakdown */}
      {modelBreakdown.length > 0 && (
        <div className="bg-card border border-border rounded-lg p-4 mb-6">
          <h2 className="text-sm font-medium mb-4">模型用量分布</h2>
          <div className="space-y-3">
            {modelBreakdown.map(({ model, tokens, pct }) => (
              <div key={model}>
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="font-mono text-xs">{model}</span>
                  <span className="text-muted-foreground text-xs">
                    {formatTokens(tokens)} ({pct}%)
                  </span>
                </div>
                <div className="w-full bg-muted rounded-full h-2">
                  <div
                    className="bg-primary rounded-full h-2 transition-all"
                    style={{ width: `${pct}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ============ Advanced Stats ============ */}

      {/* Project Token Ranking */}
      {advancedStats && advancedStats.projectTokenRanking.length > 0 && (
        <div className="bg-card border border-border rounded-lg p-4 mb-6">
          <h2 className="text-sm font-medium mb-4 flex items-center gap-2">
            <FolderOpen className="w-4 h-4" />
            项目 Token 消耗排行
          </h2>
          <div className="space-y-3">
            {advancedStats.projectTokenRanking.map((p, i) => {
              const maxTokens = advancedStats.projectTokenRanking[0]?.totalTokens || 1;
              const pct = ((p.totalTokens / maxTokens) * 100).toFixed(1);
              return (
                <div key={p.projectName}>
                  <div className="flex items-center justify-between text-sm mb-1">
                    <span className="flex items-center gap-2">
                      <span className="text-muted-foreground text-xs w-5">{i + 1}.</span>
                      <span className="font-mono text-xs">{p.projectName}</span>
                    </span>
                    <span className="text-muted-foreground text-xs">
                      {formatTokens(p.totalTokens)}
                    </span>
                  </div>
                  <div className="w-full bg-muted rounded-full h-2">
                    <div
                      className="bg-blue-500 rounded-full h-2 transition-all"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Tool Call Ranking */}
      {advancedStats && advancedStats.toolCallRanking.length > 0 && (() => {
        const TOOL_COLORS: Record<string, string> = {
          Read: "#3b82f6", Edit: "#f59e0b", Write: "#ef4444",
          Bash: "#22c55e", Grep: "#8b5cf6", Glob: "#ec4899",
          Agent: "#06b6d4", WebSearch: "#14b8a6", WebFetch: "#6366f1",
        };
        const toolData = advancedStats.toolCallRanking.map((t) => ({
          name: t.toolName,
          count: t.callCount,
          fill: TOOL_COLORS[t.toolName] || "#94a3b8",
        }));
        return (
          <div className="bg-card border border-border rounded-lg p-4 mb-6">
            <h2 className="text-sm font-medium mb-4 flex items-center gap-2">
              <Wrench className="w-4 h-4" />
              工具调用频率排行
            </h2>
            <ResponsiveContainer width="100%" height={Math.max(200, toolData.length * 28)}>
              <BarChart data={toolData} layout="vertical" margin={{ left: 80 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                <XAxis
                  type="number"
                  tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
                />
                <YAxis
                  type="category"
                  dataKey="name"
                  tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
                  width={80}
                />
                <Tooltip
                  contentStyle={{
                    backgroundColor: "hsl(var(--card))",
                    border: "1px solid hsl(var(--border))",
                    borderRadius: "6px",
                    fontSize: 12,
                  }}
                  formatter={(value: number) => [value.toLocaleString(), "调用次数"]}
                />
                <Bar dataKey="count" radius={[0, 4, 4, 0]}>
                  {toolData.map((entry, index) => (
                    <Cell key={index} fill={entry.fill} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
        );
      })()}

      {/* Session Efficiency */}
      {advancedStats && advancedStats.efficiency.totalSessions > 0 && (() => {
        const eff = advancedStats.efficiency;
        return (
          <div className="bg-card border border-border rounded-lg p-4 mb-6">
            <h2 className="text-sm font-medium mb-4 flex items-center gap-2">
              <TrendingUp className="w-4 h-4" />
              会话效率分析
            </h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
              <div className="text-center">
                <div className="text-2xl font-bold">{eff.avgMessagesPerSession}</div>
                <div className="text-xs text-muted-foreground">平均消息数/会话</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold">{formatTokens(eff.avgTokensPerSession)}</div>
                <div className="text-xs text-muted-foreground">平均 Token/会话</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold">{eff.totalSessions.toLocaleString()}</div>
                <div className="text-xs text-muted-foreground">总会话数</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold">{eff.totalMessages.toLocaleString()}</div>
                <div className="text-xs text-muted-foreground">总消息数</div>
              </div>
            </div>
            {eff.distribution.length > 0 && (
              <>
                <h3 className="text-xs text-muted-foreground mb-2">会话消息数分布</h3>
                <ResponsiveContainer width="100%" height={180}>
                  <BarChart data={eff.distribution}>
                    <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
                    <XAxis
                      dataKey="label"
                      tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
                    />
                    <YAxis
                      tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
                    />
                    <Tooltip
                      contentStyle={{
                        backgroundColor: "hsl(var(--card))",
                        border: "1px solid hsl(var(--border))",
                        borderRadius: "6px",
                        fontSize: 12,
                      }}
                      formatter={(value: number) => [value.toLocaleString(), "会话数"]}
                    />
                    <Bar dataKey="count" fill="#8b5cf6" name="会话数" radius={[4, 4, 0, 0]} />
                  </BarChart>
                </ResponsiveContainer>
              </>
            )}
          </div>
        );
      })()}
    </div>
  );
}

// ============ Codex Stats ============

function CodexStats({ stats }: { stats: CodexTokenSummary }) {
  if (!stats) return null;

  const months = useMemo(
    () => extractMonths(stats.dailyTokens.map((d) => d.date)),
    [stats]
  );
  const [selectedMonth, setSelectedMonth] = useState("all");

  const filteredDaily = useMemo(
    () => stats.dailyTokens.filter((d) =>
      selectedMonth === "all" || d.date.startsWith(selectedMonth)
    ),
    [stats, selectedMonth]
  );

  const totalInput = filteredDaily.reduce((s, d) => s + d.inputTokens, 0);
  const totalOutput = filteredDaily.reduce((s, d) => s + d.outputTokens, 0);
  const totalTokens = filteredDaily.reduce((s, d) => s + d.totalTokens, 0);

  const inputOutputRatio =
    totalOutput > 0 ? (totalInput / totalOutput).toFixed(2) : "N/A";

  const dailyData = filteredDaily.map((d) => ({
    date: d.date.slice(5),
    input: d.inputTokens,
    output: d.outputTokens,
    total: d.totalTokens,
  }));

  const modelBreakdown = selectedMonth === "all"
    ? Object.entries(stats.tokensByModel)
      .sort(([, a], [, b]) => b - a)
      .map(([model, tokens]) => ({
        model,
        tokens,
        pct: stats.totalTokens > 0 ? ((tokens / stats.totalTokens) * 100).toFixed(1) : "0",
      }))
    : [];

  return (
    <div className="p-6 max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold">使用统计</h1>
        <MonthFilter months={months} selected={selectedMonth} onChange={setSelectedMonth} />
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-8">
        <StatCard
          icon={<Calendar className="w-5 h-5" />}
          label="总会话数"
          value={stats.sessionCount.toLocaleString()}
        />
        <StatCard
          icon={<MessageSquare className="w-5 h-5" />}
          label="总消息数"
          value={stats.messageCount.toLocaleString()}
        />
        <StatCard
          icon={<Activity className="w-5 h-5" />}
          label="总 Token"
          value={formatTokens(totalTokens)}
        />
        <StatCard
          icon={<ArrowDownUp className="w-5 h-5" />}
          label="输入/输出比"
          value={inputOutputRatio}
        />
      </div>

      {/* Daily tokens stacked bar chart */}
      {dailyData.length > 0 && (
        <div className="bg-card border border-border rounded-lg p-4 mb-6">
          <h2 className="text-sm font-medium mb-4">每日 Token 用量</h2>
          <ResponsiveContainer width="100%" height={250}>
            <BarChart data={dailyData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis
                dataKey="date"
                tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
              />
              <YAxis
                tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
                tickFormatter={(v) => formatTokens(v)}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: "hsl(var(--card))",
                  border: "1px solid hsl(var(--border))",
                  borderRadius: "6px",
                  fontSize: 12,
                }}
                formatter={(value: number, name: string) => [
                  formatTokens(value),
                  name === "input" ? "输入" : "输出",
                ]}
              />
              <Bar dataKey="input" stackId="a" fill="#22c55e" name="input" radius={[0, 0, 0, 0]} />
              <Bar dataKey="output" stackId="a" fill="#16a34a" name="output" radius={[2, 2, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Total tokens trend */}
      {dailyData.length > 0 && (
        <div className="bg-card border border-border rounded-lg p-4 mb-6">
          <h2 className="text-sm font-medium mb-4">Token 用量趋势</h2>
          <ResponsiveContainer width="100%" height={200}>
            <AreaChart data={dailyData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis
                dataKey="date"
                tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
              />
              <YAxis
                tick={{ fontSize: 11, fill: "hsl(var(--muted-foreground))" }}
                tickFormatter={(v) => formatTokens(v)}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: "hsl(var(--card))",
                  border: "1px solid hsl(var(--border))",
                  borderRadius: "6px",
                  fontSize: 12,
                }}
                formatter={(value: number) => [formatTokens(value), "Tokens"]}
              />
              <Area
                type="monotone"
                dataKey="total"
                stroke="#22c55e"
                fill="#22c55e"
                fillOpacity={0.2}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Model breakdown */}
      {modelBreakdown.length > 0 && (
        <div className="bg-card border border-border rounded-lg p-4">
          <h2 className="text-sm font-medium mb-4">模型用量分布</h2>
          <div className="space-y-3">
            {modelBreakdown.map(({ model, tokens, pct }) => (
              <div key={model}>
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="font-mono text-xs">{model}</span>
                  <span className="text-muted-foreground text-xs">
                    {formatTokens(tokens)} ({pct}%)
                  </span>
                </div>
                <div className="w-full bg-muted rounded-full h-2">
                  <div
                    className="bg-green-500 rounded-full h-2 transition-all"
                    style={{ width: `${pct}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ============ Cursor / Copilot Stats ============

function CursorStatsView({ stats }: { stats: CursorStatsType }) {
  if (!stats) return null;

  return (
    <div className="p-6 max-w-6xl mx-auto">
      <h1 className="text-2xl font-bold mb-6">使用统计</h1>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
        <StatCard
          icon={<Calendar className="w-5 h-5" />}
          label="总会话数"
          value={(stats.totalSessions ?? 0).toLocaleString()}
        />
        <StatCard
          icon={<Activity className="w-5 h-5" />}
          label="总项目数"
          value={(stats.totalProjects ?? 0).toLocaleString()}
        />
        <StatCard
          icon={<MessageSquare className="w-5 h-5" />}
          label="总消息数"
          value={(stats.totalMessages ?? 0).toLocaleString()}
        />
      </div>
    </div>
  );
}

/** Extract unique "YYYY-MM" months from date strings, sorted descending */
function extractMonths(dates: string[]): string[] {
  const months = new Set<string>();
  for (const d of dates) {
    if (d.length >= 7) months.add(d.slice(0, 7));
  }
  return Array.from(months).sort().reverse();
}

/** Format "YYYY-MM" to display label like "2026年3月" */
function formatMonth(ym: string): string {
  const [y, m] = ym.split("-");
  return `${y}年${parseInt(m)}月`;
}

function MonthFilter({
  months,
  selected,
  onChange,
}: {
  months: string[];
  selected: string;
  onChange: (m: string) => void;
}) {
  const options = ["all", ...months];
  const idx = options.indexOf(selected);

  return (
    <div className="flex items-center gap-1">
      <button
        className="p-1 rounded hover:bg-muted disabled:opacity-30"
        disabled={idx >= options.length - 1}
        onClick={() => onChange(options[idx + 1])}
      >
        <ChevronLeft className="w-4 h-4" />
      </button>
      <select
        className="bg-muted border border-border rounded px-2 py-1 text-sm min-w-[120px] text-center"
        value={selected}
        onChange={(e) => onChange(e.target.value)}
      >
        <option value="all">全部</option>
        {months.map((m) => (
          <option key={m} value={m}>{formatMonth(m)}</option>
        ))}
      </select>
      <button
        className="p-1 rounded hover:bg-muted disabled:opacity-30"
        disabled={idx <= 0}
        onClick={() => onChange(options[idx - 1])}
      >
        <ChevronRight className="w-4 h-4" />
      </button>
    </div>
  );
}

function StatCard({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="bg-card border border-border rounded-lg p-4">
      <div className="flex items-center gap-2 text-muted-foreground mb-2">
        {icon}
        <span className="text-xs">{label}</span>
      </div>
      <div className="text-2xl font-bold">{value}</div>
    </div>
  );
}
