/** Shared color palette for player tiers (王者赛季制). Originally lived
 *  inside `TierBadgePill.tsx`; extracted so the Podium card's mini tier chip
 *  can render in matching colors without duplicating the table. */

export interface Palette {
  cardBorder: string;
  cardBg: string;
  medallionGradient: string;
  medallionRing: string;
  headingText: string;
  subText: string;
  gapText: string;
}

export const COLOR_PALETTE: Record<string, Palette> = {
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
