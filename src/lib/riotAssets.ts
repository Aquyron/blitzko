// Static, non-patch-specific game UI assets (position icons, ranked
// emblems) mirrored by CommunityDragon from the League client's own static
// asset bundle. Verified live (2026-09) — these paths aren't part of any
// officially published API, just the client's bundled resources.
const CDRAGON_ASSETS = "https://raw.communitydragon.org/latest/plugins/rcp-fe-lol-static-assets/global/default";

const POSITION_KEY: Record<string, string> = {
  top: "top",
  jungle: "jungle",
  mid: "middle",
  adc: "bottom",
  support: "utility",
};

export function positionIconUrl(position: string): string | undefined {
  const key = POSITION_KEY[position];
  return key ? `${CDRAGON_ASSETS}/svg/position-${key}.svg` : undefined;
}

const RANK_KEY: Record<string, string> = {
  iron: "iron",
  bronze: "bronze",
  silver: "silver",
  gold: "gold",
  platinum: "platinum",
  emerald: "emerald",
  emerald_plus: "emerald",
  diamond: "diamond",
  master: "master",
  grandmaster: "grandmaster",
  challenger: "challenger",
};

// Note: "ranked-emblem/emblem-*.png" (an earlier guess) turned out to be a
// 1280x720 banner asset with the actual crest occupying only a small
// centered fraction of the canvas — useless at any icon size. This
// "ranked-mini-crests" set has a tight viewBox matching its content,
// confirmed live via CommunityDragon's directory listing (2026-09).
export function rankIconUrl(tier: string): string | undefined {
  const key = RANK_KEY[tier];
  return key ? `${CDRAGON_ASSETS}/ranked-mini-crests/${key}.svg` : undefined;
}

// Despite the "cherry" (Arena) name, this file (fetched from "latest", so it
// always reflects the current patch) lists every augment across every mode
// — used here only to know which augment ids are actually still live, since
// OP.GG's ARAM augment list can include stale/removed entries with blank
// names and 0% stats. Verified live (2026-09): its numeric `id` matches
// OP.GG's augment `id` exactly.
const AUGMENTS_JSON_URL =
  "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/cherry-augments.json";

export type CurrentAugment = { id: number; name: string };

// Full current-patch augment list (id + name), not just ids — lets us list
// augments OP.GG hasn't computed stats for yet (confirmed live: OP.GG's
// per-champion ARAM augment stats can lag behind new augments actually
// live in the current patch) so they're at least searchable/findable,
// rather than silently missing.
export async function loadCurrentAugments(): Promise<CurrentAugment[]> {
  const res = await fetch(AUGMENTS_JSON_URL);
  const list: { id: number; nameTRA?: string }[] = await res.json();
  return list
    .filter((a) => a.nameTRA && a.nameTRA.trim() !== "")
    .map((a) => ({ id: a.id, name: a.nameTRA as string }));
}
