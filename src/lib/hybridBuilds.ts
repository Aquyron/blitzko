// Manually transcribed builds for champions where OP.GG's own site genuinely
// shows two distinct, internally-coherent builds (items AND runes matching
// each other) — but their public API only ever returns ONE tier-wide
// aggregate that mixes both playerbases together (confirmed live, 2026-09:
// OP.GG's API-reported rune page for Shaco is Hail of Blades regardless of
// tier, even though the AP build's actual keystone on their own site is
// Arcane Comet at a *better* win rate, 54% vs 48%). There is no API
// parameter to ask for "runes conditioned on this item build", so this is
// transcribed by hand from op.gg/lol/champions/shaco/build/jungle rather
// than computed — it will drift out of date as the meta shifts and needs
// re-checking against the site periodically, unlike everything else in this
// app which reads OP.GG live.
export type RuneSpec = {
  primaryTree: string;
  primaryRunes: string[]; // keystone first
  secondaryTree: string;
  secondaryRunes: string[];
};

export type HybridBuildSpec = {
  runes: RuneSpec;
  jungleItem?: string;
  coreItems: string[];
  boots: string;
  fourthOptions: string[];
  fifthOptions: string[];
  sixthOptions: string[];
  // One "Q"/"W"/"E"/"R" per level (1-18), matching the SkillGrid format
  // used for OP.GG-sourced skill orders elsewhere in this component.
  skillOrder: string[];
};

export type HybridBuilds = {
  ad: HybridBuildSpec;
  ap: HybridBuildSpec;
};

const SHACO_JUNGLE: HybridBuilds = {
  ad: {
    runes: {
      primaryTree: "Domination",
      primaryRunes: ["Hail of Blades", "Sudden Impact", "Sixth Sense", "Treasure Hunter"],
      secondaryTree: "Precision",
      secondaryRunes: ["Legend: Alacrity", "Cut Down"],
    },
    jungleItem: "Scorchclaw Pup",
    coreItems: ["Umbral Glaive", "Voltaic Cyclosword", "Boots of Swiftness"],
    boots: "Boots of Swiftness",
    fourthOptions: ["Profane Hydra", "Infinity Edge"],
    fifthOptions: ["Infinity Edge", "Lord Dominik's Regards", "The Collector"],
    sixthOptions: ["Infinity Edge", "Youmuu's Ghostblade", "Lord Dominik's Regards"],
    // Q@2,12,13,14,15 / W@1,4,5,17,18 / E@3,7,8,9,10 / R@6,11,16
    skillOrder: [
      "W", "Q", "E", "W", "W", "R", "E", "E", "E", "E",
      "R", "Q", "Q", "Q", "Q", "R", "W", "W",
    ],
  },
  ap: {
    runes: {
      primaryTree: "Sorcery",
      primaryRunes: ["Arcane Comet", "Axiom Arcanist", "Transcendence", "Gathering Storm"],
      secondaryTree: "Precision",
      secondaryRunes: ["Legend: Haste", "Cut Down"],
    },
    jungleItem: "Scorchclaw Pup",
    coreItems: ["Blackfire Torch", "Ionian Boots of Lucidity", "Liandry's Torment"],
    boots: "Ionian Boots of Lucidity",
    fourthOptions: ["Mejai's Soulstealer", "Imperial Mandate"],
    fifthOptions: ["Lord Dominik's Regards", "Cryptbloom", "Infinity Edge"],
    sixthOptions: ["Zhonya's Hourglass", "Rabadon's Deathcap", "Infinity Edge"],
    // Q@2,14,15,17,18 / W@1,3,5,7,9 / E@4,8,10,12,13 / R@6,11,16
    skillOrder: [
      "W", "Q", "W", "E", "W", "R", "W", "E", "W", "E",
      "R", "E", "E", "Q", "Q", "R", "Q", "Q",
    ],
  },
};

export const HYBRID_BUILDS: Record<string, HybridBuilds> = {
  Shaco: SHACO_JUNGLE,
};

export function hybridBuildFor(championName: string): HybridBuilds | null {
  return HYBRID_BUILDS[championName] ?? null;
}
