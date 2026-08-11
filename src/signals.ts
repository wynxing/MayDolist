import type { ActionSignal } from "./types/github";

/** Stable display labels for action signals (UI never maps raw GitHub strings). */
export const SIGNAL_LABELS: Record<ActionSignal, string> = {
  needsAction: "需要我处理",
  needsReview: "需要 Review",
  ciFailed: "CI 失败",
  stale: "长期未更新",
  draft: "Draft",
};

/** Filter options shown in the GitHub view (Draft is display-only). */
export const SIGNAL_FILTER_OPTIONS: Array<[ActionSignal, string]> = [
  ["needsAction", "需要我处理"],
  ["needsReview", "需要 Review"],
  ["ciFailed", "CI 失败"],
  ["stale", "长期未更新"],
];

export interface SignalBadge {
  key: ActionSignal;
  label: string;
}

export function signalBadges(signals: ActionSignal[] | undefined): SignalBadge[] {
  return (signals ?? []).map((signal) => ({
    key: signal,
    label: SIGNAL_LABELS[signal] ?? signal,
  }));
}
