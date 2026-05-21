import { useEffect, useMemo, useState } from "react";
import { Loader2, ChevronDown, ChevronUp, MapPin, Users } from "lucide-react";
import * as api from "../../services/tauriApi";
import type {
  RankingPayload,
  GroupRankSlice,
  GroupRankEntry,
  RankingEntry,
} from "../../services/tauriApi";
import { Podium } from "./Podium";

/** Rankings tab — individual + region + organization leaderboards, with a
 *  today/month toggle shared by all three. Data is piggybacked on the
 *  /api/report upload cycle (client is anonymous, can't call /api/stats/*
 *  directly); we read the latest snapshot from the Tauri-side state via
 *  `getLatestRanking`. Auto-refresh every 30s mirrors Podium's old behavior.
 *
 *  Falls back to a friendly "等待首次上报" hint when the server is on a
 *  pre-0.4.6 build (no today/month snapshot) — the user can still see the
 *  legacy daily podium via the bare Podium component on personal tabs. */
export function RankingsTab() {
  const [data, setData] = useState<RankingPayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [window, setWindow] = useState<"today" | "month">("today");

  useEffect(() => {
    let cancelled = false;
    const fetchOnce = async () => {
      try {
        const rk = await api.getLatestRanking();
        if (!cancelled) setData(rk);
      } catch (e) {
        console.error("[RankingsTab] fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    fetchOnce();
    const id = setInterval(fetchOnce, 30_000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground py-6 px-6">
        <Loader2 className="w-4 h-4 animate-spin" />
        加载排名数据…
      </div>
    );
  }

  if (!data) {
    return (
      <div className="px-6 py-6 text-sm text-muted-foreground">
        还没有排名数据 · 等待首次上报完成后会显示
      </div>
    );
  }

  // 0.4.6+ servers send today/month snapshots. Older servers only have the
  // flat top-level fields — fall back to a single-podium view in that case.
  const snapshot = window === "today" ? data.today : data.month;
  const fallbackOnly = !data.today && !data.month;

  if (fallbackOnly) {
    return (
      <div className="px-6 pt-6 space-y-4">
        <div className="rounded-md border border-amber-200 bg-amber-50 dark:bg-amber-950/30 dark:border-amber-900/50 px-4 py-2 text-xs text-amber-900 dark:text-amber-100">
          服务器版本较旧，仅显示今日个人排行 · 升级到 0.4.6+ 即可看到区域 / 团队排名
        </div>
        <Podium />
      </div>
    );
  }

  return (
    <div className="px-6 pt-6 space-y-6">
      <RankingsTimeToggle value={window} onChange={setWindow} />

      {snapshot ? (
        <>
          {/* Individual leaderboard — keep Podium intact (banner + 3 cards). */}
          <Podium
            snapshot={snapshot.user}
            date={snapshot.date}
            title={window === "today" ? "今日个人排行" : "本月个人排行"}
          />

          <GroupRankSection
            title="地区排行"
            emoji="🌏"
            icon={<MapPin className="w-4 h-4" />}
            slice={snapshot.region}
            windowLabel={window === "today" ? "今日" : "本月"}
          />

          <GroupRankSection
            title="团队排行"
            emoji="🏢"
            icon={<Users className="w-4 h-4" />}
            slice={snapshot.org}
            windowLabel={window === "today" ? "今日" : "本月"}
          />
        </>
      ) : (
        <div className="rounded-md border border-border bg-muted/30 px-4 py-6 text-sm text-muted-foreground text-center">
          {window === "today" ? "今日" : "本月"}还没有排名数据
        </div>
      )}
    </div>
  );
}

// ── Time toggle ────────────────────────────────────────────────────

function RankingsTimeToggle({
  value,
  onChange,
}: {
  value: "today" | "month";
  onChange: (v: "today" | "month") => void;
}) {
  const baseCls = "px-3 py-1 text-sm rounded-md transition-colors";
  const activeCls = "bg-primary text-primary-foreground font-medium";
  const idleCls = "text-muted-foreground hover:bg-accent";
  return (
    <div className="inline-flex items-center gap-1 rounded-lg border border-border bg-card p-1">
      <button
        className={`${baseCls} ${value === "today" ? activeCls : idleCls}`}
        onClick={() => onChange("today")}
      >
        今日
      </button>
      <button
        className={`${baseCls} ${value === "month" ? activeCls : idleCls}`}
        onClick={() => onChange("month")}
      >
        本月
      </button>
    </div>
  );
}

// ── Group rank section (region / organization) ───────────────────

function GroupRankSection({
  title,
  emoji,
  icon,
  slice,
  windowLabel,
}: {
  title: string;
  emoji: string;
  icon: React.ReactNode;
  slice: GroupRankSlice;
  windowLabel: string;
}) {
  const isUnclassified = slice.your_group === "未分类";
  const hasMembers = (slice.your_team_members?.length ?? 0) > 0;
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-base font-semibold flex items-center gap-2">
          {icon}
          <span>{emoji} {title}</span>
          <span className="text-xs font-normal text-muted-foreground">· {windowLabel}</span>
        </h3>
        <div className="text-xs text-muted-foreground">
          {slice.total_ranked > 0
            ? `共 ${slice.total_ranked} ${title === "地区排行" ? "个地区" : "个团队"}`
            : "暂无数据"}
        </div>
      </div>

      {slice.top3.length === 0 ? (
        <div className="rounded-lg bg-muted/40 px-4 py-6 text-center text-sm text-muted-foreground">
          {windowLabel}还没有{title}数据
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3 items-end">
          {slice.top3.map((entry, idx) => (
            <GroupRankCard
              key={entry.grp}
              entry={entry}
              costAbove={idx > 0 ? slice.top3[idx - 1].estimated_cost : null}
              costBelow={idx < slice.top3.length - 1 ? slice.top3[idx + 1].estimated_cost : null}
            />
          ))}
        </div>
      )}

      {/* Your-group placement row — always show so users see where they stand */}
      <div className="flex items-center justify-between rounded-md border border-border bg-muted/30 px-3 py-2 text-sm">
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground">你的{title === "地区排行" ? "地区" : "团队"}：</span>
          <span className={`font-medium ${isUnclassified ? "italic text-muted-foreground" : ""}`}>
            {slice.your_group}
          </span>
          {slice.your_rank != null ? (
            <span className="text-muted-foreground">
              · #{slice.your_rank} / {slice.total_ranked} · <span className="font-mono">{formatCost(slice.your_cost)}</span>
            </span>
          ) : (
            <span className="text-muted-foreground">· {windowLabel}暂无消耗</span>
          )}
        </div>
        {title === "团队排行" && hasMembers && (
          <button
            className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
            onClick={() => setExpanded((v) => !v)}
          >
            {expanded ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
            {expanded ? "收起本团队成员排名" : "查看本团队成员排名"}
          </button>
        )}
      </div>

      {title === "团队排行" && expanded && hasMembers && (
        <TeamMembersList members={slice.your_team_members ?? []} />
      )}
    </div>
  );
}

// ── Group rank card (one bucket) ────────────────────────────────

const GROUP_MEDAL_STYLE: Record<
  string,
  { bg: string; bar: string; emoji: string; label: string; order: string; raise: string }
> = {
  gold: {
    bg: "bg-gradient-to-b from-yellow-50 to-white dark:from-yellow-950/30 dark:to-background",
    bar: "bg-gradient-to-b from-yellow-400 to-yellow-600",
    emoji: "🥇",
    label: "冠军",
    order: "md:order-2",
    raise: "md:pt-5 md:pb-5",
  },
  silver: {
    bg: "bg-gradient-to-b from-slate-50 to-white dark:from-slate-900/40 dark:to-background",
    bar: "bg-gradient-to-b from-slate-300 to-slate-500",
    emoji: "🥈",
    label: "亚军",
    order: "md:order-1",
    raise: "",
  },
  bronze: {
    bg: "bg-gradient-to-b from-orange-50 to-white dark:from-orange-950/30 dark:to-background",
    bar: "bg-gradient-to-b from-orange-400 to-orange-700",
    emoji: "🥉",
    label: "季军",
    order: "md:order-3",
    raise: "",
  },
};

function GroupRankCard({
  entry,
  costAbove,
  costBelow,
}: {
  entry: GroupRankEntry;
  costAbove: number | null;
  costBelow: number | null;
}) {
  const style = GROUP_MEDAL_STYLE[entry.medal || ""] ?? GROUP_MEDAL_STYLE.gold;
  const isUnclassified = entry.grp === "未分类";
  const gapText: string | null =
    entry.rank === 1
      ? (costBelow != null ? `领先 #2 ${formatCost(entry.estimated_cost - costBelow)}` : null)
      : (costAbove != null ? `差 ${formatCost(costAbove - entry.estimated_cost)} 追 #${entry.rank - 1}` : null);
  return (
    <div
      className={`relative overflow-hidden rounded-xl border border-border ${style.bg} ${style.order} ${style.raise} px-4 py-3 shadow-sm`}
      title={`${style.label} · #${entry.rank}`}
    >
      <div className={`absolute left-0 top-0 bottom-0 w-1 ${style.bar}`} />
      <div className="flex items-center gap-2 mb-2">
        <span className={entry.medal === "gold" ? "text-3xl" : "text-2xl"}>{style.emoji}</span>
        <span className="text-[11px] font-semibold tracking-wider uppercase text-muted-foreground">
          {style.label} · #{entry.rank}
        </span>
      </div>
      <div
        className={`font-semibold leading-tight break-words ${entry.medal === "gold" ? "text-lg" : "text-base"} ${isUnclassified ? "italic text-muted-foreground" : ""}`}
      >
        {entry.grp}
      </div>
      <div className="text-xs text-muted-foreground mb-2">
        {entry.user_count != null && `${entry.user_count} 人`}
        {entry.total_tokens != null && ` · ${formatTokens(entry.total_tokens)} tokens`}
      </div>
      <div className={`font-bold text-orange-600 dark:text-orange-400 ${entry.medal === "gold" ? "text-2xl" : "text-xl"}`}>
        {formatCost(entry.estimated_cost)}
      </div>
      {entry.message_count != null && (
        <div className="text-xs text-muted-foreground mt-1">
          {entry.message_count.toLocaleString()} 条消息
        </div>
      )}
      {gapText && (
        <div className="text-[11px] mt-1.5 inline-block px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
          {gapText}
        </div>
      )}
    </div>
  );
}

// ── Team members list ──────────────────────────────────────────

function TeamMembersList({ members }: { members: RankingEntry[] }) {
  const medalEmoji = useMemo(() => ({ gold: "🥇", silver: "🥈", bronze: "🥉" } as Record<string, string>), []);
  return (
    <div className="rounded-md border border-border bg-card divide-y divide-border">
      {members.map((m) => {
        const display = m.remark || m.name || m.email;
        const showEmail = display !== m.email;
        const medal = m.medal ? medalEmoji[m.medal] : null;
        return (
          <div key={m.email} className="flex items-center justify-between px-3 py-2 text-sm">
            <div className="flex items-center gap-2 min-w-0">
              <span className="w-10 text-center font-mono text-muted-foreground">
                {medal ?? `#${m.rank}`}
              </span>
              <span className="font-medium truncate">{display}</span>
              {showEmail && (
                <span className="text-xs text-muted-foreground truncate">{m.email}</span>
              )}
            </div>
            <div className="flex items-center gap-3 text-xs text-muted-foreground shrink-0">
              <span>{formatTokens(m.total_tokens)} tokens</span>
              {m.message_count != null && <span>{m.message_count.toLocaleString()} 条</span>}
              <span className="font-mono font-semibold text-orange-600 dark:text-orange-400">
                {formatCost(m.estimated_cost)}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ── helpers ────────────────────────────────────────────────────

function formatCost(usd: number): string {
  if (usd >= 1) return `$${usd.toFixed(2)}`;
  return `$${usd.toFixed(4)}`;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}
