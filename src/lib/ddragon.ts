import { invoke } from "@tauri-apps/api/core";

const CDN = "https://ddragon.leagueoflegends.com";

export type DdragonIndex = {
  version: string;
  championByName: Map<string, { id: string; key: string }>;
  itemByName: Map<string, string>;
  spellByKey: Map<string, { name: string; image: string }>;
  spellByName: Map<string, { image: string; key: string }>;
  runeByName: Map<string, { icon: string; id: number }>;
  runeStyleByName: Map<string, { icon: string; id: number }>;
};

export async function loadDdragonIndex(): Promise<DdragonIndex> {
  const [version, champions, items, spells, runes] = await Promise.all([
    invoke<string>("get_ddragon_version"),
    invoke<any>("get_champion_list"),
    invoke<any>("get_item_list"),
    invoke<any>("get_summoner_spell_list"),
    invoke<any>("get_rune_list"),
  ]);

  const championByName = new Map<string, { id: string; key: string }>();
  for (const c of Object.values<any>(champions.data)) {
    championByName.set(c.name, { id: c.id, key: c.key });
  }

  // Data Dragon lists the same display name multiple times across game
  // modes (e.g. "Health Potion" as the normal SR item 2003 and again as an
  // Arena-mode variant with a much larger id). Keep the canonical
  // (lowest-id) entry so icon lookups don't randomly resolve to an
  // off-mode variant.
  const itemByName = new Map<string, string>();
  for (const [id, item] of Object.entries<any>(items.data)) {
    const existing = itemByName.get(item.name);
    if (!existing || Number(id) < Number(existing)) {
      itemByName.set(item.name, id);
    }
  }

  const spellByKey = new Map<string, { name: string; image: string }>();
  // Same trap as items: Data Dragon lists a spell like "Flash" multiple
  // times across game-mode variants (e.g. key 74 is a reskinned "Jade"-mode
  // Flash with a completely different icon) — keep the canonical
  // (lowest-key) entry so an enemy scouted mid-game doesn't show some
  // limited-mode reskin instead of the spell everyone actually recognizes.
  const spellByName = new Map<string, { image: string; key: string }>();
  for (const spell of Object.values<any>(spells.data)) {
    spellByKey.set(spell.key, { name: spell.name, image: spell.image.full });
    const existing = spellByName.get(spell.name);
    if (!existing || Number(spell.key) < Number(existing.key)) {
      spellByName.set(spell.name, { image: spell.image.full, key: spell.key });
    }
  }

  const runeByName = new Map<string, { icon: string; id: number }>();
  const runeStyleByName = new Map<string, { icon: string; id: number }>();
  for (const style of runes as any[]) {
    runeStyleByName.set(style.name, { icon: style.icon, id: style.id });
    for (const slot of style.slots) {
      for (const rune of slot.runes) {
        runeByName.set(rune.name, { icon: rune.icon, id: rune.id });
      }
    }
  }

  return {
    version,
    championByName,
    itemByName,
    spellByKey,
    spellByName,
    runeByName,
    runeStyleByName,
  };
}

export async function loadChampionIdIndex(): Promise<Map<number, string>> {
  const champions = await invoke<any>("get_champion_list");
  const map = new Map<number, string>();
  for (const c of Object.values<any>(champions.data)) {
    map.set(Number(c.key), c.name);
  }
  return map;
}

export function championIconUrl(idx: DdragonIndex, championName: string): string | undefined {
  const c = idx.championByName.get(championName);
  return c ? `${CDN}/cdn/${idx.version}/img/champion/${c.id}.png` : undefined;
}

// OP.GG's `champion` argument wants Data Dragon's champion id in
// SCREAMING_SNAKE_CASE (e.g. "MonkeyKing" -> "MONKEY_KING"), not the
// display name — shared by every place that calls `get_champion_build`.
export function opggChampionKey(idx: DdragonIndex, championName: string): string {
  const c = idx.championByName.get(championName);
  const ddragonId = c ? c.id : championName;
  return ddragonId.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toUpperCase();
}

export function itemIconUrl(idx: DdragonIndex, itemName: string): string | undefined {
  const id = idx.itemByName.get(itemName);
  return id ? `${CDN}/cdn/${idx.version}/img/item/${id}.png` : undefined;
}

export function spellIconUrl(idx: DdragonIndex, spellKey: number): string | undefined {
  const spell = idx.spellByKey.get(String(spellKey));
  return spell ? `${CDN}/cdn/${idx.version}/img/spell/${spell.image}` : undefined;
}

// Live client data only exposes a summoner spell's display name (e.g.
// "Flash"), not its numeric key — this is the only source we have for
// looking up an already-in-game enemy's spells.
export function spellIconUrlByName(idx: DdragonIndex, spellName: string): string | undefined {
  const spell = idx.spellByName.get(spellName);
  return spell ? `${CDN}/cdn/${idx.version}/img/spell/${spell.image}` : undefined;
}

export function spellName(idx: DdragonIndex, spellKey: number): string {
  return idx.spellByKey.get(String(spellKey))?.name ?? `Spell ${spellKey}`;
}

export function runeIconUrl(idx: DdragonIndex, runeName: string): string | undefined {
  const rune = idx.runeByName.get(runeName) ?? idx.runeStyleByName.get(runeName);
  return rune ? `${CDN}/cdn/img/${rune.icon}` : undefined;
}

// Rune shard (stat mod) perks aren't part of Data Dragon's runesReforged.json
// at all — Riot only exposes them through the LCU's live /lol-perks/v1/perks
// endpoint. Confirmed live against a running client (2026-09); this small,
// stable set rarely changes so it's hardcoded rather than fetched.
const STAT_MODS: Record<number, { name: string; icon: string }> = {
  5001: { name: "Health Scaling", icon: "StatModsHealthPlusIcon.png" },
  5002: { name: "Armor", icon: "StatModsArmorIcon.png" },
  5003: { name: "Magic Resist", icon: "StatModsMagicResIcon.png" },
  5005: { name: "Attack Speed", icon: "StatModsAttackSpeedIcon.png" },
  5007: { name: "Ability Haste", icon: "StatModsCDRScalingIcon.png" },
  5008: { name: "Adaptive Force", icon: "StatModsAdaptiveForceIcon.png" },
  5010: { name: "Move Speed", icon: "StatModsMovementSpeedIcon.png" },
  5011: { name: "Health", icon: "StatModsHealthScalingIcon.png" },
  5012: { name: "Resist Scaling", icon: "StatModsAdaptiveForceScalingIcon.png" },
  5013: { name: "Tenacity and Slow Resist", icon: "StatModsTenacityIcon.png" },
};

export function statModName(id: number): string {
  return STAT_MODS[id]?.name ?? `Stat ${id}`;
}

export function statModIconUrl(id: number): string | undefined {
  const mod = STAT_MODS[id];
  return mod ? `${CDN}/cdn/img/perk-images/StatMods/${mod.icon}` : undefined;
}
