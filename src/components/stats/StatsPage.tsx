import { useEffect } from "react";
import { useAppStore } from "../../stores/appStore";
import type { StatsCache, ClaudeTokenSummary, CodexTokenSummary } from "../../types";
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
} from "recharts";
import {
  MessageSquare,
  Zap,
  Activity,
  Loader2,
  Calendar,
  ArrowDownUp,
} from "lucide-react";

export function StatsPage() {
  const { activeTool, stats, tokenSummary, statsLoading, loadStats } =
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

  return (
    <ClaudeStats
      stats={stats as StatsCache}
      tokenSummary={tokenSummary as ClaudeTokenSummary | null}
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
}: {
  stats: StatsCache;
  tokenSummary: ClaudeTokenSummary | null;
}) {
  if (!stats) return null;

  const totalMessages = stats.dailyActivity.reduce(
    (sum, d) => sum + d.messageCount,
    0
  );
  const totalSessions = stats.dailyActivity.reduce(
    (sum, d) => sum + d.sessionCount,
    0
  );
  const totalToolCalls = stats.dailyActivity.reduce(
    (sum, d) => sum + d.toolCallCount,
    0
  );

  const activityData = stats.dailyActivity.map((d) => ({
    date: d.date.slice(5),
    messages: d.messageCount,
    sessions: d.sessionCount,
    tools: d.toolCallCount,
  }));

  const tokenData =
    tokenSummary?.dailyTokens.map((d) => ({
      date: d.date.slice(5),
      input: d.inputTokens,
      output: d.outputTokens,
      total: d.totalTokens,
    })) || [];

  const modelBreakdown = tokenSummary
    ? Object.entries(tokenSummary.tokensByModel)
      .sort(([, a], [, b]) => b - a)
      .map(([model, tokens]) => ({
        model: model.replace("claude-", "").replace(/-\d+$/, ""),
        tokens,
        pct: ((tokens / tokenSummary.totalTokens) * 100).toFixed(1),
      }))
    : [];

  return (
    <div className="p-6 max-w-6xl mx-auto">
      <h1 className="text-2xl font-bold mb-6">使用统计</h1>

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
          value={formatTokens(tokenSummary?.totalInputTokens || 0)}
        />
        <StatCard
          icon={<Activity className="w-5 h-5" />}
          label="输出 Token"
          value={formatTokens(tokenSummary?.totalOutputTokens || 0)}
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
                    className="bg-primary rounded-full h-2 transition-all"
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

// ============ Codex Stats ============

function CodexStats({ stats }: { stats: CodexTokenSummary }) {
  if (!stats) return null;

  const inputOutputRatio =
    stats.totalOutputTokens > 0
      ? (stats.totalInputTokens / stats.totalOutputTokens).toFixed(2)
      : "N/A";

  const dailyData = stats.dailyTokens.map((d) => ({
    date: d.date.slice(5),
    input: d.inputTokens,
    output: d.outputTokens,
    total: d.totalTokens,
  }));

  const modelBreakdown = Object.entries(stats.tokensByModel)
    .sort(([, a], [, b]) => b - a)
    .map(([model, tokens]) => ({
      model,
      tokens,
      pct: stats.totalTokens > 0 ? ((tokens / stats.totalTokens) * 100).toFixed(1) : "0",
    }));

  return (
    <div className="p-6 max-w-6xl mx-auto">
      <h1 className="text-2xl font-bold mb-6">使用统计</h1>

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
          value={formatTokens(stats.totalTokens)}
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
