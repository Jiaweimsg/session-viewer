import type { TierInfo } from "../../services/tauriApi";

/** Compact tier pill — fits on the same row as the StatsPage tab bar. Same
 *  visual cues as the full TierBadge (medallion gradient + emoji + label +
 *  cost-to-next), but condensed into a single 36-ish-px-tall capsule so it
 *  never pushes the tab bar to wrap. */
export function TierBadgePill({ tier }: { tier: TierInfo }) {
  const palette = COLOR_PALETTE[tier.color] ?? COLOR_PALETTE.bronze;
  const isKing = tier.color === "king" || tier.color === "legend";
  const gap =
    tier.next_label && tier.cost_to_next != null
      ? `离${tier.next_label} 差 ${formatCost(tier.cost_to_next)}`
      : "已封顶 · 无人能及";
  return (
    <div
      className={`inline-flex items-center gap-2 rounded-full border ${palette.cardBorder} ${palette.cardBg} pl-1 pr-3 py-1 shadow-sm`}
      title={`本月段位 · 本月消费 ${formatCost(tier.current_cost)}`}
    >
      <Medallion palette={palette} emoji={tier.emoji} pulse={isKing} />
      <div className="flex items-center gap-1.5 min-w-0">
        <span className={`text-sm font-bold whitespace-nowrap ${palette.headingText}`}>
          {tier.label}
        </span>
        {tier.sub && (
          <span className={`text-xs font-semibold ${palette.subText}`}>{tier.sub}</span>
        )}
        <span className="text-muted-foreground/70 text-xs">·</span>
        <span className={`text-[11px] whitespace-nowrap ${palette.gapText}`}>{gap}</span>
        <span className="text-muted-foreground text-[11px] whitespace-nowrap">
          · {formatCost(tier.current_cost)}
        </span>
      </div>
    </div>
  );
}

function Medallion({
  palette,
  emoji,
  pulse,
}: {
  palette: Palette;
  emoji: string;
  pulse: boolean;
}) {
  return (
    <div
      className={`relative w-7 h-7 rounded-full flex items-center justify-center shrink-0 ${palette.medallionRing} ${pulse ? "animate-pulse" : ""}`}
      style={{ background: palette.medallionGradient }}
    >
      <span
        className="text-sm select-none"
        style={{ filter: "drop-shadow(0 1px 1px rgba(0,0,0,0.4))" }}
      >
        {emoji}
      </span>
      <span
        className="absolute inset-0 rounded-full pointer-events-none"
        style={{
          background:
            "radial-gradient(circle at 30% 25%, rgba(255,255,255,0.55) 0%, rgba(255,255,255,0) 45%)",
        }}
      />
    </div>
  );
}

function formatCost(usd: number): string {
  if (usd >= 1) return `$${usd.toFixed(2)}`;
  return `$${usd.toFixed(4)}`;
}

interface Palette {
  cardBorder: string;
  cardBg: string;
  medallionGradient: string;
  medallionRing: string;
  headingText: string;
  subText: string;
  gapText: string;
}

const COLOR_PALETTE: Record<string, Palette> = {
  bronze: {
    cardBorder: "border-amber-200 dark:border-amber-900/40",
    cardBg: "bg-gradient-to-r from-amber-50 to-orange-50/40 dark:from-amber-950/30 dark:to-orange-950/10",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #fed7aa 0%, #d97706 45%, #78350f 100%)",
    medallionRing: "ring-1 ring-amber-700/50",
    headingText: "text-amber-800 dark:text-amber-200",
    subText: "text-amber-700/80 dark:text-amber-300/80",
    gapText: "text-amber-700 dark:text-amber-300",
  },
  silver: {
    cardBorder: "border-slate-200 dark:border-slate-700",
    cardBg: "bg-gradient-to-r from-slate-50 to-white dark:from-slate-900/50 dark:to-slate-900/20",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #f8fafc 0%, #cbd5e1 45%, #475569 100%)",
    medallionRing: "ring-1 ring-slate-400/60",
    headingText: "text-slate-700 dark:text-slate-200",
    subText: "text-slate-500 dark:text-slate-400",
    gapText: "text-slate-600 dark:text-slate-300",
  },
  gold: {
    cardBorder: "border-yellow-200 dark:border-yellow-900/40",
    cardBg: "bg-gradient-to-r from-yellow-50 to-amber-50/40 dark:from-yellow-950/30 dark:to-amber-950/10",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #fef9c3 0%, #facc15 40%, #a16207 100%)",
    medallionRing: "ring-1 ring-yellow-500/70",
    headingText: "text-yellow-700 dark:text-yellow-300",
    subText: "text-yellow-600/80 dark:text-yellow-400/80",
    gapText: "text-amber-600 dark:text-yellow-300",
  },
  platinum: {
    cardBorder: "border-teal-200 dark:border-teal-900/40",
    cardBg: "bg-gradient-to-r from-teal-50 to-cyan-50/40 dark:from-teal-950/30 dark:to-cyan-950/10",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #cffafe 0%, #22d3ee 40%, #0e7490 100%)",
    medallionRing: "ring-1 ring-teal-400/60",
    headingText: "text-teal-700 dark:text-teal-300",
    subText: "text-teal-600/80 dark:text-teal-400/80",
    gapText: "text-teal-600 dark:text-teal-300",
  },
  diamond: {
    cardBorder: "border-sky-200 dark:border-sky-900/40",
    cardBg: "bg-gradient-to-r from-sky-50 to-blue-50/40 dark:from-sky-950/30 dark:to-blue-950/10",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #e0f2fe 0%, #38bdf8 40%, #1e40af 100%)",
    medallionRing: "ring-1 ring-sky-400/70",
    headingText: "text-sky-700 dark:text-sky-300",
    subText: "text-sky-600/80 dark:text-sky-400/80",
    gapText: "text-sky-700 dark:text-sky-300",
  },
  starshine: {
    cardBorder: "border-purple-200 dark:border-purple-900/40",
    cardBg: "bg-gradient-to-r from-purple-50 to-pink-50/40 dark:from-purple-950/30 dark:to-pink-950/10",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #fbcfe8 0%, #c084fc 40%, #6b21a8 100%)",
    medallionRing: "ring-1 ring-purple-400/70",
    headingText: "text-purple-700 dark:text-purple-300",
    subText: "text-purple-600/80 dark:text-purple-400/80",
    gapText: "text-purple-700 dark:text-purple-300",
  },
  king: {
    cardBorder: "border-orange-300 dark:border-orange-900/50",
    cardBg: "bg-gradient-to-r from-amber-50 via-orange-50 to-red-50/40 dark:from-amber-950/40 dark:via-orange-950/30 dark:to-red-950/20",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #fef3c7 0%, #f59e0b 35%, #b91c1c 100%)",
    medallionRing: "ring-1 ring-orange-500/80",
    headingText: "text-orange-700 dark:text-orange-300",
    subText: "text-orange-600/80 dark:text-orange-400/80",
    gapText: "text-red-600 dark:text-orange-300",
  },
  legend: {
    cardBorder: "border-red-300 dark:border-red-900/50",
    cardBg: "bg-gradient-to-r from-red-50 via-amber-50 to-yellow-50/40 dark:from-red-950/40 dark:via-amber-950/30 dark:to-yellow-950/20",
    medallionGradient:
      "radial-gradient(circle at 35% 30%, #fef08a 0%, #f97316 35%, #7f1d1d 100%)",
    medallionRing: "ring-1 ring-red-500/80",
    headingText: "text-red-700 dark:text-amber-300",
    subText: "text-red-600/80 dark:text-amber-400/80",
    gapText: "text-red-700 dark:text-amber-300",
  },
};
