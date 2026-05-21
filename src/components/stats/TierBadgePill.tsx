import type { TierInfo } from "../../services/tauriApi";
import { COLOR_PALETTE, type Palette } from "./tierPalette";

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
