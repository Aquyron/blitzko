// The five current-season finished support items all branch from the same
// "Bounty of Worlds" upgrade — which one to buy is a champion-kit judgment
// call (which passive fits how the champion plays), not something OP.GG's
// official API tracks: confirmed live (2026-09) across three separate
// OP.GG MCP tools (lol_get_champion_analysis, lol_list_champion_details,
// lol_list_items) that none of them expose support-item stats at all, even
// though OP.GG's own website shows a "Support items" section — a real gap
// between their website and their public API, same category of gap found
// earlier for Alistar's augments.
//
// So this is a knowledge-based recommendation (verified against each
// item's actual current-patch passive text below), not a stats-driven one:
// - Dream Maker: procs bonus damage + a damage-reduction shield whenever
//   you heal/shield an ally -> enchanters (heal/shield kits).
// - Zaz'Zak's Realmspike: ability damage triggers an AoE explosion ->
//   mage/poke supports.
// - Celestial Opposition: reactive shield + slow after taking champion
//   damage -> tanky engage supports who soak damage.
// - Solstice Sleigh: slowing/rooting an enemy heals + speeds you and a
//   nearby ally -> proactive CC/catcher supports.
// - Bloodsong: on-hit bonus physical damage after using an ability ->
//   AD/attack-based supports.
export type SupportItemKey = "dreamMaker" | "zazZak" | "celestialOpposition" | "solsticeSleigh" | "bloodsong";

export const SUPPORT_ITEM_NAMES: Record<SupportItemKey, string> = {
  dreamMaker: "Dream Maker",
  zazZak: "Zaz'Zak's Realmspike",
  celestialOpposition: "Celestial Opposition",
  solsticeSleigh: "Solstice Sleigh",
  bloodsong: "Bloodsong",
};

const CHAMPION_SUPPORT_ITEM: Record<string, SupportItemKey> = {
  // Enchanters
  Nami: "dreamMaker",
  Soraka: "dreamMaker",
  Janna: "dreamMaker",
  Lulu: "dreamMaker",
  Karma: "dreamMaker",
  Milio: "dreamMaker",
  Yuumi: "dreamMaker",
  "Renata Glasc": "dreamMaker",
  Sona: "dreamMaker",
  Rakan: "dreamMaker",
  Seraphine: "dreamMaker",

  // Mage / poke
  Xerath: "zazZak",
  Zyra: "zazZak",
  "Vel'Koz": "zazZak",
  Brand: "zazZak",
  Lux: "zazZak",
  Morgana: "zazZak",
  Ziggs: "zazZak",
  Neeko: "zazZak",
  Swain: "zazZak",
  Zilean: "zazZak",
  Vex: "zazZak",

  // Tanky engage / peel (reactive)
  Leona: "celestialOpposition",
  Nautilus: "celestialOpposition",
  Alistar: "celestialOpposition",
  Rell: "celestialOpposition",
  Braum: "celestialOpposition",
  Taric: "celestialOpposition",
  Poppy: "celestialOpposition",
  Maokai: "celestialOpposition",

  // Catcher / proactive CC
  Thresh: "solsticeSleigh",
  Blitzcrank: "solsticeSleigh",
  Skarner: "solsticeSleigh",

  // AD / on-hit supports
  Pyke: "bloodsong",
  Senna: "bloodsong",
  Vayne: "bloodsong",
  Twitch: "bloodsong",
};

export function supportItemFor(championName: string): { key: SupportItemKey; name: string } | null {
  const key = CHAMPION_SUPPORT_ITEM[championName];
  if (!key) return null;
  return { key, name: SUPPORT_ITEM_NAMES[key] };
}
