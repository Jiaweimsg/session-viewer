import { useEffect, useState } from "react";
import { Trophy, Loader2 } from "lucide-react";
import * as api from "../../services/tauriApi";
import type { RankingPayload, UserRankSlice } from "../../services/tauriApi";

/** Daily gold/silver/bronze podium for the whole team — same data the
 *  dashboard shows, but piggybacked on the local report-usage cycle so we
 *  don't need a second auth'd HTTP call.
 *
 *  Two modes:
 *  - Uncontrolled (no props): self-polls the Tauri-side ranking state every
 *    30s. Used when Podium stands alone (legacy callers).
 *  - Controlled (`snapshot` + `date` passed in): renders the given slice
 *    directly. Used by RankingsTab to swap between today/month snapshots
 *    without each card re-issuing IPC calls.
 *
 *  The "today's leaderboard" was originally piggybacked to avoid a separate
 *  auth'd call — that contract hasn't changed. */
export function Podium({
  snapshot,
  date,
  title,
}: {
  snapshot?: UserRankSlice | null;
  date?: string;
  title?: string;
} = {}) {
  const controlled = snapshot !== undefined;
  const [polled, setPolled] = useState<RankingPayload | null>(null);
  const [loading, setLoading] = useState(!controlled);

  useEffect(() => {
    if (controlled) return; // controlled mode: parent owns the data
    let cancelled = false;
    const fetchOnce = async () => {
      try {
        const rk = await api.getLatestRanking();
        if (!cancelled) setPolled(rk);
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
  }, [controlled]);

  // Compose an effective slice in the controlled-or-not branch into a single
  // shape the rendering code below can consume uniformly.
  const effective: { date: string; slice: UserRankSlice } | null = (() => {
    if (controlled) {
      if (!snapshot) return null;
      return { date: date ?? "", slice: snapshot };
    }
    if (!polled) return null;
    // Prefer the v0.4.6+ today.user slice when available; fall back to the
    // legacy top-level fields so servers ≤ 0.4.5 still render.
    const slice: UserRankSlice = polled.today?.user ?? {
      top3: polled.top3,
      your_rank: polled.your_rank,
      your_cost: polled.your_cost,
      your_next_cost: polled.your_next_cost,
      your_next_name: polled.your_next_name,
      your_chaser_cost: polled.your_chaser_cost,
      your_chaser_name: polled.your_chaser_name,
      total_ranked: polled.total_ranked,
    };
    return { date: polled.date, slice };
  })();

  if (loading) {
    return (
      <div className="flex items-center gap-2 text-sm text-muted-foreground py-4">
        <Loader2 className="w-4 h-4 animate-spin" />
        加载{title ?? "今日排行"}…
      </div>
    );
  }

  if (!effective || effective.slice.top3.length === 0) {
    return (
      <div className="rounded-lg bg-muted/40 px-4 py-6 text-center text-sm text-muted-foreground">
        {title ?? "今日"}还没人上榜 · 上报完成后会显示排行
      </div>
    );
  }

  const { date: effDate, slice } = effective;

  return (
    <div className="mb-6 space-y-3">
      <YourRankBanner slice={slice} date={effDate} title={title} />
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3 items-end">
        {slice.top3.map((entry, idx) => (
          <PodiumCard
            key={entry.email}
            entry={entry}
            costAbove={idx > 0 ? slice.top3[idx - 1].estimated_cost : null}
            costBelow={idx < slice.top3.length - 1 ? slice.top3[idx + 1].estimated_cost : null}
          />
        ))}
      </div>
    </div>
  );
}

function YourRankBanner({ slice, date, title }: { slice: UserRankSlice; date: string; title?: string }) {
  const hasRank = slice.your_rank != null;
  // "Above" chip = catch-up target. Three cases:
  //   - rank 1: lead over rank 2 (use top3[1] for both cost and name)
  //   - rank > 1: server-provided your_next_cost + your_next_name
  //   - no rank: nothing to compare
  // Falls back to "#N-1" / "#2" when a neighbor name isn't piggybacked
  // (older server, or future shape change) — keeps banner readable.
  let gapText: string | null = null;
  if (slice.your_rank === 1) {
    const runnerUp = slice.top3[1];
    if (runnerUp != null) {
      const runnerUpName = displayName(runnerUp.remark, runnerUp.name, runnerUp.email);
      gapText = `👑 领先 ${shortName(runnerUpName)} $${(slice.your_cost - runnerUp.estimated_cost).toFixed(2)}`;
    }
  } else if (slice.your_rank != null && slice.your_next_cost != null) {
    const aboveLabel = shortName(slice.your_next_name) ?? `#${slice.your_rank - 1}`;
    gapText = `⚠️ 仅差 ${formatCost(slice.your_next_cost - slice.your_cost)} 反超 ${aboveLabel}`;
  }
  // "Chaser" chip = the person right behind, breathing down your neck.
  // Renders only when there IS a chaser; missing on last place or no rank.
  let chaserText: string | null = null;
  if (hasRank && slice.your_chaser_cost != null) {
    const chaserLabel = shortName(slice.your_chaser_name) ?? `#${(slice.your_rank as number) + 1}`;
    chaserText = `🔥 ${chaserLabel} 紧咬不放 仅差 ${formatCost(slice.your_cost - slice.your_chaser_cost)}`;
  }
  return (
    <div className="flex items-center justify-between rounded-lg border border-amber-200 bg-gradient-to-r from-amber-50 to-yellow-50 dark:from-amber-950/30 dark:to-yellow-950/30 dark:border-amber-900/50 px-4 py-2.5 text-sm">
      <div className="flex items-center gap-2">
        <Trophy className="w-4 h-4 text-amber-600" />
        <span className="font-medium text-amber-900 dark:text-amber-200">
          {title ?? "今日排行"} · {date}
        </span>
      </div>
      <div className="text-amber-800 dark:text-amber-200 flex items-center gap-3">
        {hasRank ? (
          <>
            <span>
              你排第 <span className="font-bold text-base">#{slice.your_rank}</span>
              <span className="mx-1">/</span>
              <span className="text-amber-700/80">{slice.total_ranked}</span>
              <span className="mx-2">·</span>
              <span className="font-mono">{formatCost(slice.your_cost)}</span>
            </span>
            {gapText && (
              <span className="text-xs px-2 py-0.5 rounded bg-amber-200/60 dark:bg-amber-800/40 text-amber-900 dark:text-amber-100">
                {gapText}
              </span>
            )}
            {chaserText && (
              <span className="text-xs px-2 py-0.5 rounded bg-amber-200/60 dark:bg-amber-800/40 text-amber-900 dark:text-amber-100">
                {chaserText}
              </span>
            )}
          </>
        ) : (
          <span className="text-muted-foreground">{title ?? "今日"}你还没消耗记录</span>
        )}
      </div>
    </div>
  );
}

/** Pick the best human-readable name from a ranking entry — same priority
 *  Podium cards use (`Podium.tsx:157`). Falls back to email when no overlay. */
function displayName(remark: string | null, name: string | null, email: string): string {
  return remark || name || email;
}

/** Truncate a name with `…` so the banner doesn't get blown out by a long
 *  remark or full email. Null/undefined passes through so the caller can
 *  fall back to a `#rank` label. */
function shortName(s: string | null | undefined, max = 12): string | null {
  if (s == null) return null;
  return s.length > max ? `${s.slice(0, max - 1)}…` : s;
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
