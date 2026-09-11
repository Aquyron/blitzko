export type Confidence = "low" | "medium" | "high";

const LOW_THRESHOLD = 200;
const HIGH_THRESHOLD = 1000;

export function confidenceFor(games: number): Confidence {
  if (games < LOW_THRESHOLD) return "low";
  if (games < HIGH_THRESHOLD) return "medium";
  return "high";
}

export function winRate(win: number, play: number): number {
  return play > 0 ? (win / play) * 100 : 0;
}

export function formatGames(games: number): string {
  return games >= 1000 ? `${(games / 1000).toFixed(1)}k` : String(games);
}

// Both champion tier (lol_list_lane_meta_champions) and augment tier
// (lol_list_aram_augments) are OP.GG's own 1-5 graded judgment on that
// data (1=best), not just a raw stat to re-rank ourselves — map it to a
// letter directly so what we show matches what their own site shows for
// the same champion/augment, rather than inventing our own ranking.
export function gradeFromTier(tier: number): string {
  if (tier <= 1) return "S";
  if (tier === 2) return "A";
  if (tier === 3) return "B";
  if (tier === 4) return "C";
  return "D";
}

export function gradeColorVar(grade: string): string {
  switch (grade) {
    case "S+":
    case "S":
      return "var(--green)";
    case "A":
      return "var(--accent)";
    case "B":
      return "var(--yellow)";
    default:
      return "var(--red)";
  }
}
