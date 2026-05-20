import { useEffect, useState } from "react";
import { Trophy, Loader2 } from "lucide-react";
import * as api from "../../services/tauriApi";
import type { RankingPayload } from "../../services/tauriApi";

/** Daily gold/silver/bronze podium for the whole team — same data the
 *  dashboard shows, but piggybacked on the local report-usage cycle so we
 *  don't need a second auth'd HTTP call.
 *
 *  The payload is refreshed by the background auto-report loop (every 5 min)
 *  AND any time the user triggers a manual report. Here we just poll the
 *  Tauri-side state once on mount and again on a slow interval so users see
 *  the updated leaderboard without restarting the app.
 */
export function Podium() {
  const [data, setData] = useState<RankingPayload | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const fetchOnce = async () => {
      try {
        const rk = await api.getLatestRanking();
        if (!cancelled) setData(rk);
      } catch (e) {
        console.error("[Podium] fetch failed:", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    fetchOnce();
    // Slow refresh — auto-report writes to state every 5 min, so a 30-s
    // poll catches updates without spamming Tauri's IPC.
    const id = setInterval(fetchOnce, 30_000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground py-4">
        <Loader2 className="w-4 h-4 animate-spin" />
        加载今日排行…
      </div>
    );
  }

  if (!data || data.top3.length === 0) {
    return (
      <div className="rounded-lg bg-muted/40 px-4 py-6 text-center text-sm text-muted-foreground">
        今日还没人上榜 · 上报完成后会显示排行
      </div>
    );
  }

  return (
    <div className="mb-6 space-y-3">
      <YourRankBanner data={data} />
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3 items-end">
        {data.top3.map((entry, idx) => (
          <PodiumCard
            key={entry.email}
            entry={entry}
            costAbove={idx > 0 ? data.top3[idx - 1].estimated_cost : null}
            costBelow={idx < data.top3.length - 1 ? data.top3[idx + 1].estimated_cost : null}
          />
        ))}
      </div>
    </div>
  );
}

function YourRankBanner({ data }: { data: RankingPayload }) {
  const hasRank = data.your_rank != null;
  // Gap-to-next-rank text. Three cases:
  //   - rank 1: lead over rank 2 (use top3[1] if present)
  //   - rank > 1: server-provided your_next_cost
  //   - no rank: nothing to compare
  let gapText: string | null = null;
  if (data.your_rank === 1) {
    const runnerUp = data.top3[1]?.estimated_cost;
    if (runnerUp != null) gapText = `领先 #2 ${formatCost(data.your_cost - runnerUp)}`;
  } else if (data.your_rank != null && data.your_next_cost != null) {
    gapText = `差 ${formatCost(data.your_next_cost - data.your_cost)} 追上 #${data.your_rank - 1}`;
  }
  return (
    <div className="flex items-center justify-between rounded-lg border border-amber-200 bg-gradient-to-r from-amber-50 to-yellow-50 dark:from-amber-950/30 dark:to-yellow-950/30 dark:border-amber-900/50 px-4 py-2.5 text-sm">
      <div className="flex items-center gap-2">
        <Trophy className="w-4 h-4 text-amber-600" />
        <span className="font-medium text-amber-900 dark:text-amber-200">
          今日排行 · {data.date}
        </span>
      </div>
      <div className="text-amber-800 dark:text-amber-200 flex items-center gap-3">
        {hasRank ? (
          <>
            <span>
              你排第 <span className="font-bold text-base">#{data.your_rank}</span>
              <span className="mx-1">/</span>
              <span className="text-amber-700/80">{data.total_ranked}</span>
              <span className="mx-2">·</span>
              <span className="font-mono">{formatCost(data.your_cost)}</span>
            </span>
            {gapText && (
              <span className="text-xs px-2 py-0.5 rounded bg-amber-200/60 dark:bg-amber-800/40 text-amber-900 dark:text-amber-100">
                {gapText}
              </span>
            )}
          </>
        ) : (
          <span className="text-muted-foreground">今日你还没消耗记录</span>
        )}
      </div>
    </div>
  );
}

const MEDAL_STYLE: Record<string, { bg: string; bar: string; emoji: string; label: string; order: string; raise: string }> = {
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

function PodiumCard({
  entry,
  costAbove,
  costBelow,
}: {
  entry: import("../../services/tauriApi").RankingEntry;
  costAbove: number | null;
  costBelow: number | null;
}) {
  const style = MEDAL_STYLE[entry.medal || ""] ?? MEDAL_STYLE.gold;
  const displayName = entry.remark || entry.name || entry.email;
  const showEmail = displayName !== entry.email;
  // Gap-to-next-rank: gold shows lead over silver; silver/bronze show
  // catch-up distance to the rank above. Null when no neighbor exists.
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
        <span className={entry.medal === "gold" ? "text-3xl" : "text-2xl"}>
          {style.emoji}
        </span>
        <span className="text-[11px] font-semibold tracking-wider uppercase text-muted-foreground">
          {style.label} · #{entry.rank}
        </span>
      </div>
      <div className={`font-semibold leading-tight break-words ${entry.medal === "gold" ? "text-lg" : "text-base"}`}>
        {displayName}
      </div>
      {showEmail && (
        <div className="text-xs text-muted-foreground break-all mb-2">
          {entry.email}
        </div>
      )}
      <div className={`font-bold text-orange-600 dark:text-orange-400 ${entry.medal === "gold" ? "text-2xl" : "text-xl"}`}>
        {formatCost(entry.estimated_cost)}
      </div>
      <div className="text-xs text-muted-foreground mt-1">
        {formatTokens(entry.total_tokens)} tokens
        {entry.message_count != null && ` · ${entry.message_count.toLocaleString()} 条`}
      </div>
      {gapText && (
        <div className="text-[11px] mt-1.5 inline-block px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
          {gapText}
        </div>
      )}
    </div>
  );
}

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
