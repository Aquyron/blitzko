import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  DdragonIndex,
  championIconUrl,
  itemIconUrl,
  loadDdragonIndex,
  opggChampionKey,
  runeIconUrl,
  spellIconUrl,
  spellName,
  statModIconUrl,
  statModName,
} from "../lib/ddragon";
import {
  confidenceFor,
  formatGames,
  gradeFromTier,
  gradeColorVar,
  winRate,
} from "../lib/recommendation";
import { CurrentAugment, loadCurrentAugments, rankIconUrl } from "../lib/riotAssets";
import { supportItemFor } from "../lib/supportItems";
import { hybridBuildFor } from "../lib/hybridBuilds";
import { RolePicker } from "./RolePicker";
import "./ChampionLookup.css";

// OP.GG's own schema lists "all"/"none" as valid positions, but the live
// API rejects both unconditionally (confirmed 2026-09) — every call needs a
// concrete lane, so it's simply not offered as a choice.
const POSITIONS = ["top", "jungle", "mid", "adc", "support"];
const POSITION_LABELS: Record<string, string> = {
  top: "Top",
  jungle: "Jungle",
  mid: "Mid",
  adc: "ADC",
  support: "Support",
};
// Modes with no real lane assignment (ARAM/URF/Nexus Blitz) still require
// *some* valid position value server-side even though it has no effect on
// the result — confirmed live by comparing responses across positions.
const LANE_MODES = new Set(["ranked", "flex"]);


function RankBadgeIcon({ tier }: { tier: string }) {
  const url = rankIconUrl(tier);
  if (!url) {
    return <span className="role-icon-fallback">ALL</span>;
  }
  return <img src={url} alt={tier} className="rank-icon-img" />;
}
const TIERS = [
  "all",
  "iron",
  "bronze",
  "silver",
  "gold",
  "platinum",
  "emerald_plus",
  "diamond",
  "master",
  "grandmaster",
  "challenger",
];
const MODES = ["ranked", "flex", "aram", "urf", "nexus_blitz"];
const ABILITIES = ["Q", "W", "E", "R"] as const;
const MAX_LEVEL = 18;

// OP.GG's skill priority data stops at level 15 — the remaining 3 points
// are mechanically forced by Riot's leveling rules (R only levels at 6/11/16,
// and by level 15 exactly one of Q/W/E is still short of rank 5), so we
// finish the sequence ourselves instead of leaving it visually truncated.
function extendSkillOrder(order: string[]): string[] {
  const result = [...order];
  const counts: Record<string, number> = { Q: 0, W: 0, E: 0, R: 0 };
  for (const a of result) counts[a] = (counts[a] ?? 0) + 1;
  for (let level = result.length + 1; level <= 18; level++) {
    if ([6, 11, 16].includes(level) && counts.R < 3) {
      result.push("R");
      counts.R += 1;
      continue;
    }
    const next = ["Q", "W", "E"].find((a) => counts[a] < 5);
    if (next) {
      result.push(next);
      counts[next] += 1;
    }
  }
  return result;
}

type ItemGroup = { ids_names?: string[]; play?: number; win?: number };
type Counter = {
  champion_name?: string;
  play?: number;
  my_win_rate?: number;
  counter_win_rate?: number;
};
type Augment = {
  id?: number;
  name?: string;
  desc?: string;
  tier?: number;
  performance?: number;
  popular?: number;
};
type AramAugmentsResponse = { data?: { augments?: Augment[] } };

function stripTags(html: string): string {
  return html.replace(/<[^>]+>/g, "");
}

type Analysis = {
  data?: {
    summary?: {
      average_stats?: {
        play?: number;
        win_rate?: number;
        pick_rate?: number;
        ban_rate?: number;
        tier_data?: { tier?: number };
      };
    };
    starter_items?: ItemGroup;
    core_items?: ItemGroup;
    boots?: ItemGroup;
    fourth_items?: ItemGroup[];
    fifth_items?: ItemGroup[];
    sixth_items?: ItemGroup[];
    runes?: {
      primary_page_id?: number;
      primary_page_name?: string;
      primary_rune_ids?: number[];
      primary_rune_names?: string[];
      secondary_page_id?: number;
      secondary_page_name?: string;
      secondary_rune_ids?: number[];
      secondary_rune_names?: string[];
      stat_mod_names?: number[];
      play?: number;
      win?: number;
    };
    summoner_spells?: { ids?: number[]; play?: number; win?: number };
    skills?: { order?: string[] };
    strong_counters?: Counter[];
    weak_counters?: Counter[];
    // Server-injected — only present when strong/weak counters come back
    // empty because this champion+position+tier combo simply doesn't have
    // enough matchup data yet (common at Master+ for less-played picks).
    // Confirmed live (2026-09): shaped as {message}, not a plain string.
    counters_meta?: { message?: string };
  };
};

function ItemRow({ idx, group, label }: { idx: DdragonIndex; group?: ItemGroup; label: string }) {
  if (!group?.ids_names?.length) return null;
  const games = group.play ?? 0;
  const wr = winRate(group.win ?? 0, games);
  const conf = confidenceFor(games);
  return (
    <div className="build-row">
      <span className="build-row-label">{label}</span>
      <div className="icon-row">
        {group.ids_names.map((name, i) => {
          const url = itemIconUrl(idx, name);
          return url ? (
            <img key={i} src={url} alt={name} title={name} className="icon icon-item" />
          ) : (
            <span key={i} className="icon-fallback" title={name}>
              {name}
            </span>
          );
        })}
      </div>
      <span className={`stat conf-${conf}`}>
        {wr.toFixed(1)}% &middot; {formatGames(games)} games
      </span>
    </div>
  );
}

// Same look as ItemRow, but for the manually curated hybrid-build override
// (src/lib/hybridBuilds.ts) where we have no real play/win numbers to show.
function ManualItemRow({
  idx,
  names,
  label,
}: {
  idx: DdragonIndex;
  names: string[];
  label: string;
}) {
  if (!names.length) return null;
  return (
    <div className="build-row">
      <span className="build-row-label">{label}</span>
      <div className="icon-row">
        {names.map((name, i) => {
          const url = itemIconUrl(idx, name);
          return url ? (
            <img key={i} src={url} alt={name} title={name} className="icon icon-item" />
          ) : (
            <span key={i} className="icon-fallback" title={name}>
              {name}
            </span>
          );
        })}
      </div>
    </div>
  );
}

function SupportItemRow({ idx, champion }: { idx: DdragonIndex; champion: string }) {
  const pick = supportItemFor(champion);
  if (!pick) return null;
  const url = itemIconUrl(idx, pick.name);
  return (
    <div className="build-row">
      <span className="build-row-label">SUPPORT ITEM</span>
      <div className="icon-row">
        {url ? (
          <img src={url} alt={pick.name} title={pick.name} className="icon icon-item" />
        ) : (
          <span className="icon-fallback">{pick.name}</span>
        )}
        <span className="stat">{pick.name}</span>
      </div>
    </div>
  );
}

function ItemOptionsRow({
  idx,
  groups,
  label,
}: {
  idx: DdragonIndex;
  groups?: ItemGroup[];
  label: string;
}) {
  const options = (groups ?? []).filter((g) => g.ids_names?.[0]);
  if (!options.length) return null;

  const withWr = options.map((g) => ({ g, wr: winRate(g.win ?? 0, g.play ?? 0) }));
  const ranked = [...withWr].sort((a, b) => b.wr - a.wr);

  return (
    <div className="build-row">
      <span className="build-row-label">{label}</span>
      <div className="icon-row">
        {withWr.map(({ g, wr }, i) => {
          const name = g.ids_names![0];
          const url = itemIconUrl(idx, name);
          const rank = ranked.findIndex((o) => o.g === g);
          const rankClass =
            rank === 0 ? "rank-best" : rank === ranked.length - 1 ? "rank-worst" : "rank-mid";
          return (
            <div key={i} className="item-option">
              {url ? (
                <img src={url} alt={name} title={name} className="icon icon-item" />
              ) : (
                <span className="icon-fallback">{name}</span>
              )}
              <span className={`stat-sm ${rankClass}`}>{wr.toFixed(0)}%</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function SkillGrid({ order }: { order: string[] }) {
  return (
    <div className="skill-grid">
      {ABILITIES.map((ability) => (
        <div key={ability} className="skill-grid-row">
          <span className={`skill-grid-label skill-${ability}`}>{ability}</span>
          <div className="skill-grid-levels">
            {Array.from({ length: MAX_LEVEL }, (_, i) => {
              const level = i + 1;
              const filled = order[i] === ability;
              return (
                <span
                  key={level}
                  className={`skill-grid-cell${filled ? ` filled skill-${ability}` : ""}`}
                >
                  {filled ? level : ""}
                </span>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

export default function ChampionLookup({
  autoChampion,
  autoPosition,
  autoGameMode,
  gameflowPhase,
}: {
  autoChampion?: string;
  autoPosition?: string;
  autoGameMode?: string;
  gameflowPhase?: string;
}) {
  const [ddragon, setDdragon] = useState<DdragonIndex | null>(null);
  const [champions, setChampions] = useState<string[]>([]);
  const [champion, setChampion] = useState("Ahri");
  const [position, setPosition] = useState("mid");
  // Manually curated override for champions where OP.GG's own site shows
  // two genuinely different, internally-coherent builds but their public
  // API only reports one tier-wide aggregate that mixes both together (see
  // src/lib/hybridBuilds.ts). Defaults to whichever style the hardcoded
  // data lists first; only rendered when hybridBuild is non-null below.
  const [manualStyle, setManualStyle] = useState<"ad" | "ap">("ap");
  // Which physical key (D or F) gets the *first* summoner spell OP.GG lists
  // — purely a personal keybind preference, so it's remembered across
  // sessions rather than reset every time.
  const [spellsSwapped, setSpellsSwapped] = useState(() => {
    const stored = localStorage.getItem("blitzko_spells_swapped");
    return stored === null ? true : stored === "true";
  });
  function toggleSpellsSwapped() {
    setSpellsSwapped((prev) => {
      const next = !prev;
      localStorage.setItem("blitzko_spells_swapped", String(next));
      // Push immediately instead of waiting for the next render's stale
      // `spellsSwapped` closure to catch up — the whole point of the
      // button is an instant in-client swap, not a delayed one.
      applySpells(undefined, next);
      return next;
    });
  }
  // The hardcoded build is jungle-specific (that's the only role we
  // transcribed it for) — anyone playing Shaco elsewhere should still see
  // OP.GG's real per-position stats instead of a jungle build slapped onto
  // the wrong lane.
  const hybridBuild = position === "jungle" ? hybridBuildFor(champion) : null;
  const activeManualBuild = hybridBuild ? hybridBuild[manualStyle] : null;
  const [tier, setTier] = useState("emerald_plus");
  const [gameMode, setGameMode] = useState("ranked");
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [applyStatus, setApplyStatus] = useState<"idle" | "applying" | "success" | "error">(
    "idle"
  );
  const [applyMessage, setApplyMessage] = useState<string | null>(null);
  const [augments, setAugments] = useState<Augment[]>([]);
  const [augmentSearch, setAugmentSearch] = useState("");
  const [augmentDataCount, setAugmentDataCount] = useState(0);

  // ARAM-only: augment tiers aren't picked at champ select, they come up
  // mid-game — the augment draft pauses for input, so surfacing this here
  // (to alt-tab to) covers it without needing a live in-game overlay.
  const currentAugmentsRef = useRef<CurrentAugment[] | null>(null);
  useEffect(() => {
    setAugmentSearch("");
    if (gameMode !== "aram" || !ddragon) {
      setAugments([]);
      setAugmentDataCount(0);
      return;
    }
    const c = ddragon.championByName.get(champion);
    if (!c) return;
    (async () => {
      try {
        if (!currentAugmentsRef.current) {
          currentAugmentsRef.current = await loadCurrentAugments();
        }
        const currentList = currentAugmentsRef.current;
        const res = await invoke<AramAugmentsResponse>("get_aram_augments", {
          championId: Number(c.key),
        });
        // OP.GG's list can include stale/removed augments (blank name, 0%
        // stats) — filter those to the current patch's real stats. But
        // OP.GG's per-champion stats can also lag behind new augments that
        // are already live (confirmed: some brand-new augments return no
        // entry at all for a given champion) — so merge in every current
        // augment, even ones OP.GG has no data for yet, rather than
        // silently dropping them. Those just show "No data yet" and are
        // still searchable by name.
        const statsById = new Map(
          (res.data?.augments ?? [])
            .filter((a) => a.id !== undefined && a.name?.trim() && (a.performance ?? 0) > 0)
            .map((a) => [a.id as number, a])
        );
        const withData: Augment[] = [];
        const withoutData: Augment[] = [];
        for (const c of currentList) {
          const stats = statsById.get(c.id);
          if (stats) {
            withData.push(stats);
          } else {
            withoutData.push({ id: c.id, name: c.name });
          }
        }
        withData.sort(
          (a, b) =>
            (a.tier ?? 99) - (b.tier ?? 99) || (b.performance ?? 0) - (a.performance ?? 0)
        );
        withoutData.sort((a, b) => (a.name ?? "").localeCompare(b.name ?? ""));
        setAugmentDataCount(withData.length);
        setAugments([...withData, ...withoutData]);
      } catch {
        setAugments([]);
        setAugmentDataCount(0);
      }
    })();
  }, [gameMode, champion, ddragon]);

  // Champion select detection (League client) drives these when active —
  // still just seeding local state, so the pickers stay overridable by hand.
  useEffect(() => {
    if (autoChampion) setChampion(autoChampion);
  }, [autoChampion]);

  useEffect(() => {
    if (autoPosition && POSITIONS.includes(autoPosition)) setPosition(autoPosition);
  }, [autoPosition]);

  useEffect(() => {
    if (autoGameMode && MODES.includes(autoGameMode)) setGameMode(autoGameMode);
  }, [autoGameMode]);

  // Auto-apply runes+spells once per live champ-select pick — never for
  // manual browsing (changing champion/tier/mode by hand never sets this).
  // Debounced 300ms on top: champ select hovering flicks through several
  // champions before locking one in, and we only want to push a client
  // change once you actually rest on one, not on every fleeting hover. Was
  // 1000ms — combined with the (now-removed) redundant duplicate fetch this
  // made the client visibly lag behind the moment you actually settled on a
  // champion.
  const autoApplyPendingRef = useRef(false);
  const lastAutoKeyRef = useRef<string | null>(null);
  const autoApplyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // `load` is recreated every render (closes over current champion/position);
  // keep a ref pointed at the latest one so the timeout below never fires
  // a stale closure from the render where it was scheduled.
  const loadRef = useRef<() => void>(() => {});

  useEffect(() => {
    if (!autoChampion) return;
    const key = `${autoChampion}|${autoPosition ?? ""}`;

    if (autoApplyTimerRef.current) clearTimeout(autoApplyTimerRef.current);
    autoApplyTimerRef.current = setTimeout(() => {
      if (lastAutoKeyRef.current === key) return;
      lastAutoKeyRef.current = key;
      autoApplyPendingRef.current = true;
      loadRef.current();
    }, 300);

    return () => {
      if (autoApplyTimerRef.current) clearTimeout(autoApplyTimerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoChampion, autoPosition]);

  // OP.GG's own analysis endpoint is the actual bottleneck (measured 4+
  // seconds per call, live, regardless of session/caching on our end) — the
  // 300ms settle-debounce above is negligible next to that. The one real
  // lever we have is starting the request earlier: the instant champ select
  // shows a hover/pick, not 300ms after it "settles". Most picks are hovered
  // for several seconds before being locked in, so this often finishes
  // before lock-in and the real load below then hits `fetchAnalysis`'s
  // in-flight cache instead of waiting out another 4s round trip.
  useEffect(() => {
    if (!ddragon || !autoChampion) return;
    const pos = autoPosition && POSITIONS.includes(autoPosition) ? autoPosition : position;
    prefetchAnalysis(autoChampion, pos);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ddragon, autoChampion, autoPosition]);

  useEffect(() => {
    loadDdragonIndex().then((idx) => {
      setDdragon(idx);
      setChampions([...idx.championByName.keys()].sort());
    });
  }, []);

  // Auto-load whenever the selection changes, instead of requiring a manual
  // click every time — a light debounce keeps quick dropdown flipping from
  // firing a request per intermediate value. Skipped for a champ-select-
  // driven change (champion/position still match the live auto-detected
  // values): the dedicated auto-apply effect below already loads that exact
  // same build shortly after — firing both meant two redundant network
  // round trips racing each other, which is what made runes/spells land in
  // the client noticeably slower than they needed to.
  const prevTierRef = useRef(tier);
  const prevGameModeRef = useRef(gameMode);

  useEffect(() => {
    if (!ddragon) return;
    // Only the champion/position seeding effects above should be treated as
    // "auto-driven" (and skipped here to avoid the double-fetch race with
    // the dedicated auto-apply effect) — a manual tier or mode change must
    // always reload, even while champion/position still match the live
    // champ-select pick. Without this, switching to Diamond/Challenger
    // tier mid champ-select silently did nothing and the client kept
    // whatever tier (usually the emerald_plus default) had already been
    // auto-applied.
    const tierOrModeChanged =
      tier !== prevTierRef.current || gameMode !== prevGameModeRef.current;
    prevTierRef.current = tier;
    prevGameModeRef.current = gameMode;
    const isAutoDriven =
      !tierOrModeChanged &&
      autoChampion === champion &&
      (!autoPosition || autoPosition === position);
    if (isAutoDriven) return;
    const handle = setTimeout(() => {
      load();
    }, 250);
    return () => clearTimeout(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ddragon, champion, position, tier, gameMode]);

  // Coalesces concurrent requests for the same (champion, position, tier,
  // mode): the hover-triggered prefetch and the debounced "real" load often
  // land within the same multi-second OP.GG round trip, and without this
  // they'd fire two identical, independently-billed requests instead of
  // sharing one.
  const inFlightRef = useRef(new Map<string, Promise<Analysis>>());
  function fetchAnalysis(
    opggChampion: string,
    effectivePosition: string,
    tierArg: string,
    gameModeArg: string
  ): Promise<Analysis> {
    const key = `${opggChampion}|${effectivePosition}|${tierArg}|${gameModeArg}`;
    const existing = inFlightRef.current.get(key);
    if (existing) return existing;
    const promise = invoke<Analysis>("get_champion_build", {
      champion: opggChampion,
      position: effectivePosition,
      tier: tierArg,
      gameMode: gameModeArg,
    }).finally(() => {
      inFlightRef.current.delete(key);
    });
    inFlightRef.current.set(key, promise);
    return promise;
  }

  // Fire-and-forget cache warm for a hovered/picked champion — never touches
  // loading/error/analysis state, so it can't flash the UI for a champion
  // the player is just browsing past.
  async function prefetchAnalysis(champ: string, pos: string) {
    if (!ddragon) return;
    const opggChampion = opggChampionKey(ddragon, champ);
    const effectivePosition = LANE_MODES.has(gameMode) ? pos : "mid";
    try {
      await fetchAnalysis(opggChampion, effectivePosition, tier, gameMode);
    } catch {
      // Best-effort only — the real load will retry and surface any error.
    }
  }

  async function load() {
    if (!ddragon) return;
    setLoading(true);
    setError(null);
    try {
      const opggChampion = opggChampionKey(ddragon, champion);
      const effectivePosition = LANE_MODES.has(gameMode) ? position : "mid";
      const result = await fetchAnalysis(opggChampion, effectivePosition, tier, gameMode);
      setAnalysis(result);
      setApplyStatus("idle");

      if (autoApplyPendingRef.current) {
        autoApplyPendingRef.current = false;
        applyRunes(result);
        applySpells(result);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadRef.current = load;
  });

  async function applySpells(source?: Analysis, swappedOverride?: boolean) {
    const ids = (source ?? analysis)?.data?.summoner_spells?.ids;
    if (!ids || ids.length < 2) return;
    const swapped = swappedOverride ?? spellsSwapped;
    const [first, second] = swapped ? [ids[1], ids[0]] : [ids[0], ids[1]];
    try {
      await invoke("apply_summoner_spells", { spell1Id: first, spell2Id: second });
    } catch (e) {
      console.error("apply_summoner_spells failed", e);
    }
  }

  // Item Set push (shop recommendation, not auto-buy) once the game
  // actually starts — fires once on the None/ChampSelect -> InProgress
  // transition, not on every render while in-game.
  const lastGameflowPhaseRef = useRef<string | null>(null);
  useEffect(() => {
    if (gameflowPhase === "InProgress" && lastGameflowPhaseRef.current !== "InProgress") {
      applyItemSet();
    }
    lastGameflowPhaseRef.current = gameflowPhase ?? null;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gameflowPhase]);

  // Modes with no real lane (ARAM/URF/Nexus Blitz) always fall back to
  // "mid" internally just to satisfy OP.GG's API — showing that leftover
  // "Mid" in the applied page/item-set name was confusing ("Blitzko: Xerath
  // Mid" in an ARAM game you never picked a lane for), so the label uses
  // the actual mode instead of the meaningless position for those.
  function buildLabel(): string {
    const suffix = LANE_MODES.has(gameMode)
      ? POSITION_LABELS[position] ?? position
      : gameMode.toUpperCase();
    return `Blitzko: ${champion} ${suffix}`;
  }

  async function applyItemSet() {
    if (!ddragon) return;
    const c = ddragon.championByName.get(champion);
    if (!c) return;
    const championId = Number(c.key);

    const resolveNames = (names: string[]) =>
      names
        .map((name) => ddragon.itemByName.get(name))
        .filter((id): id is string => Boolean(id));

    const supportItemIds: string[] = [];
    if (position === "support") {
      const pick = supportItemFor(champion);
      const id = pick && ddragon.itemByName.get(pick.name);
      if (id) supportItemIds.push(id);
    }

    let blocks: { blockType: string; itemIds: string[] }[];
    if (activeManualBuild) {
      // The hardcoded build, not OP.GG's raw (unconditioned, possibly
      // wrong-style) aggregate — this is the whole point of the AD/AP
      // toggle, and applying the OP.GG data here instead was the bug where
      // the AD build never actually reached the in-game shop.
      blocks = [
        { blockType: "Support Item", itemIds: supportItemIds },
        {
          blockType: "Jungle Item",
          itemIds: activeManualBuild.jungleItem ? resolveNames([activeManualBuild.jungleItem]) : [],
        },
        { blockType: "Core Build", itemIds: resolveNames(activeManualBuild.coreItems) },
        { blockType: "Boots", itemIds: resolveNames([activeManualBuild.boots]) },
        { blockType: "4th Item Options", itemIds: resolveNames(activeManualBuild.fourthOptions) },
        { blockType: "5th Item Options", itemIds: resolveNames(activeManualBuild.fifthOptions) },
        { blockType: "6th Item Options", itemIds: resolveNames(activeManualBuild.sixthOptions) },
      ];
    } else {
      if (!analysis) return;
      const toItemIds = (group?: ItemGroup) => resolveNames(group?.ids_names ?? []);
      const toItemIdsFromOptions = (groups?: ItemGroup[]) =>
        resolveNames(
          (groups ?? []).map((g) => g.ids_names?.[0]).filter((name): name is string => Boolean(name))
        );
      blocks = [
        { blockType: "Support Item", itemIds: supportItemIds },
        { blockType: "Starting Items", itemIds: toItemIds(analysis.data?.starter_items) },
        { blockType: "Core Build", itemIds: toItemIds(analysis.data?.core_items) },
        { blockType: "Boots", itemIds: toItemIds(analysis.data?.boots) },
        { blockType: "4th Item Options", itemIds: toItemIdsFromOptions(analysis.data?.fourth_items) },
        { blockType: "5th Item Options", itemIds: toItemIdsFromOptions(analysis.data?.fifth_items) },
        { blockType: "6th Item Options", itemIds: toItemIdsFromOptions(analysis.data?.sixth_items) },
      ];
    }

    try {
      await invoke("apply_item_set", {
        title: buildLabel(),
        championId,
        blocks,
      });
    } catch (e) {
      console.error("apply_item_set failed", e);
    }
  }

  async function applyRunes(source?: Analysis) {
    // Per explicit user direction: never push a rune page for ARAM.
    if (gameMode === "aram") return;
    if (activeManualBuild) {
      // Same bug as applyItemSet: this must use the hardcoded AD/AP page,
      // not OP.GG's raw (unconditioned) rune aggregate, or the toggle is a
      // no-op on what actually gets pushed to the client.
      if (!ddragon) return;
      const spec = activeManualBuild.runes;
      const primaryStyleId = ddragon.runeStyleByName.get(spec.primaryTree)?.id;
      const secondaryStyleId = ddragon.runeStyleByName.get(spec.secondaryTree)?.id;
      const primaryIds = spec.primaryRunes
        .map((name) => ddragon.runeByName.get(name)?.id)
        .filter((id): id is number => id !== undefined);
      const secondaryIds = spec.secondaryRunes
        .map((name) => ddragon.runeByName.get(name)?.id)
        .filter((id): id is number => id !== undefined);
      if (!primaryStyleId || !secondaryStyleId || primaryIds.length === 0) return;
      setApplyStatus("applying");
      setApplyMessage(null);
      try {
        const selectedPerkIds = [...primaryIds, ...secondaryIds, ...spec.statModIds];
        const pageName = buildLabel();
        await invoke("apply_runes", {
          pageName,
          primaryStyleId,
          subStyleId: secondaryStyleId,
          selectedPerkIds,
        });
        setApplyStatus("success");
      } catch (e) {
        setApplyStatus("error");
        setApplyMessage(String(e));
      }
      return;
    }

    const runes = (source ?? analysis)?.data?.runes;
    if (!runes?.primary_page_id || !runes.primary_rune_ids || !runes.secondary_page_id) {
      return;
    }
    setApplyStatus("applying");
    setApplyMessage(null);
    try {
      const selectedPerkIds = [
        ...runes.primary_rune_ids,
        ...(runes.secondary_rune_ids ?? []),
        ...(runes.stat_mod_names ?? []),
      ];
      const pageName = buildLabel();
      await invoke("apply_runes", {
        pageName,
        primaryStyleId: runes.primary_page_id,
        subStyleId: runes.secondary_page_id,
        selectedPerkIds,
      });
      setApplyStatus("success");
    } catch (e) {
      setApplyStatus("error");
      setApplyMessage(String(e));
    }
  }

  const stats = analysis?.data?.summary?.average_stats;
  const runes = analysis?.data?.runes;
  const spells = analysis?.data?.summoner_spells;
  const skills = analysis?.data?.skills?.order;
  const strong = analysis?.data?.strong_counters ?? [];
  const weak = analysis?.data?.weak_counters ?? [];

  const champIcon = useMemo(
    () => (ddragon ? championIconUrl(ddragon, champion) : undefined),
    [ddragon, champion]
  );

  return (
    <div className="lookup">
      <div className="lookup-controls">
        <div className="controls-row">
          <select value={champion} onChange={(e) => setChampion(e.target.value)}>
            {champions.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
          <select value={gameMode} onChange={(e) => setGameMode(e.target.value)}>
            {MODES.map((m) => (
              <option key={m} value={m}>
                {m.toUpperCase()}
              </option>
            ))}
          </select>
          {loading && <span className="stat loading-indicator">Loading...</span>}
        </div>

        <div className={`controls-row${LANE_MODES.has(gameMode) ? "" : " controls-row-disabled"}`}>
          <RolePicker options={POSITIONS} value={position} onChange={setPosition} />
          {!LANE_MODES.has(gameMode) && <span className="stat">No lanes in this mode</span>}
        </div>

        <div className="controls-row">
          <div className="role-picker">
            {TIERS.map((t) => (
              <button
                key={t}
                type="button"
                title={t.replace("_", " ").toUpperCase()}
                className={`rank-icon-btn${tier === t ? " active" : ""}`}
                onClick={() => setTier(t)}
              >
                <RankBadgeIcon tier={t} />
              </button>
            ))}
          </div>
        </div>

        {hybridBuild && (
          <div className="controls-row">
            <div className="build-style-toggle build-style-toggle-lg">
              <button
                type="button"
                className={`build-style-toggle-btn ad${manualStyle === "ad" ? " active" : ""}`}
                onClick={() => setManualStyle("ad")}
              >
                AD Build
              </button>
              <button
                type="button"
                className={`build-style-toggle-btn ap${manualStyle === "ap" ? " active" : ""}`}
                onClick={() => setManualStyle("ap")}
              >
                AP Build
              </button>
            </div>
          </div>
        )}

      </div>

      {error && <p className="error-text">{error}</p>}

      {ddragon && analysis && (
        <div className="lookup-results">
          <div className="champ-header">
            {champIcon && <img src={champIcon} alt={champion} className="icon icon-champion" />}
            <div className="champ-header-info">
              <h2>{champion}</h2>
              {stats && (
                <span className="stat">
                  WR {((stats.win_rate ?? 0) * 100).toFixed(1)}% &middot; Pick{" "}
                  {((stats.pick_rate ?? 0) * 100).toFixed(1)}% &middot; Ban{" "}
                  {((stats.ban_rate ?? 0) * 100).toFixed(1)}% &middot;{" "}
                  {formatGames(stats.play ?? 0)} games &middot; Tier{" "}
                  {stats.tier_data?.tier !== undefined
                    ? gradeFromTier(stats.tier_data.tier)
                    : "?"}
                </span>
              )}
            </div>
            <div className="apply-build">
              <button
                type="button"
                className="apply-build-btn"
                disabled={applyStatus === "applying" || !analysis.data?.runes?.primary_rune_ids}
                onClick={() => {
                  applyRunes();
                  applySpells();
                }}
              >
                {applyStatus === "applying" ? "Applying..." : "Apply Runes"}
              </button>
              {applyStatus === "success" && <span className="apply-ok">Runes applied</span>}
              {applyStatus === "error" && (
                <span className="apply-fail">{applyMessage ?? "Failed to apply"}</span>
              )}
            </div>
          </div>

          <div className="lookup-body">
            <div className="lookup-col">
              {position === "support" && <SupportItemRow idx={ddragon} champion={champion} />}
              {!activeManualBuild && (
                <ItemRow idx={ddragon} group={analysis.data?.starter_items} label="STARTING ITEMS" />
              )}
              {activeManualBuild ? (
                <>
                  {activeManualBuild.jungleItem && (
                    <ManualItemRow
                      idx={ddragon}
                      names={[activeManualBuild.jungleItem]}
                      label="JUNGLE ITEM"
                    />
                  )}
                  <ManualItemRow idx={ddragon} names={activeManualBuild.coreItems} label="CORE BUILD" />
                  <ManualItemRow idx={ddragon} names={[activeManualBuild.boots]} label="BOOTS" />
                  <ManualItemRow
                    idx={ddragon}
                    names={activeManualBuild.fourthOptions}
                    label="4TH ITEM OPTIONS"
                  />
                  <ManualItemRow
                    idx={ddragon}
                    names={activeManualBuild.fifthOptions}
                    label="5TH ITEM OPTIONS"
                  />
                  <ManualItemRow
                    idx={ddragon}
                    names={activeManualBuild.sixthOptions}
                    label="6TH ITEM OPTIONS"
                  />
                </>
              ) : (
                <>
                  <ItemRow idx={ddragon} group={analysis.data?.core_items} label="CORE BUILD" />
                  <ItemRow idx={ddragon} group={analysis.data?.boots} label="BOOTS" />
                  <ItemOptionsRow idx={ddragon} groups={analysis.data?.fourth_items} label="4TH ITEM OPTIONS" />
                  <ItemOptionsRow idx={ddragon} groups={analysis.data?.fifth_items} label="5TH ITEM OPTIONS" />
                  <ItemOptionsRow idx={ddragon} groups={analysis.data?.sixth_items} label="6TH ITEM OPTIONS" />
                </>
              )}
            </div>

            <div className="lookup-col">
              {spells?.ids && spells.ids.length >= 2 && (
                <>
                  <div className="spell-swap-row">
                    <button
                      type="button"
                      className="spell-swap-btn"
                      title="Swap which key (D/F) each spell goes on"
                      onClick={toggleSpellsSwapped}
                    >
                      <span className="spell-swap-icon">⇄</span> Swap
                    </button>
                  </div>
                  <div className="build-row">
                  <span className="build-row-label">SUMMONER SPELLS</span>
                  <div className="icon-row">
                    {(spellsSwapped ? [spells.ids[1], spells.ids[0]] : [spells.ids[0], spells.ids[1]]).map(
                      (id, i) => {
                        const url = spellIconUrl(ddragon, id);
                        const name = spellName(ddragon, id);
                        return url ? (
                          <img
                            key={i}
                            src={url}
                            alt={name}
                            title={name}
                            className="icon icon-spell"
                          />
                        ) : (
                          <span key={i} className="icon-fallback">
                            {name}
                          </span>
                        );
                      }
                    )}
                  </div>
                  </div>
                </>
              )}

              {gameMode !== "aram" && (activeManualBuild ? (
                <div className="build-row">
                  <span className="build-row-label">RUNES</span>
                  <div className="rune-page">
                    <div className="rune-col rune-col-primary">
                      {activeManualBuild.runes.primaryRunes.map((name, i) => {
                        const url = runeIconUrl(ddragon, name);
                        return url ? (
                          <img
                            key={i}
                            src={url}
                            alt={name}
                            title={name}
                            className={`icon ${i === 0 ? "icon-rune-keystone" : "icon-rune"}`}
                          />
                        ) : (
                          <span key={i} className="icon-fallback">
                            {name}
                          </span>
                        );
                      })}
                    </div>
                    <div className="rune-col rune-col-secondary">
                      {activeManualBuild.runes.secondaryRunes.map((name, i) => {
                        const url = runeIconUrl(ddragon, name);
                        return url ? (
                          <img key={i} src={url} alt={name} title={name} className="icon icon-rune" />
                        ) : (
                          <span key={i} className="icon-fallback">
                            {name}
                          </span>
                        );
                      })}
                    </div>
                  </div>
                </div>
              ) : (
                runes && (
                  <div className="build-row">
                    <span className="build-row-label">RUNES</span>
                    <div className="rune-page">
                      <div className="rune-col rune-col-primary">
                        {runes.primary_rune_names?.map((name, i) => {
                          const url = runeIconUrl(ddragon, name);
                          return url ? (
                            <img
                              key={i}
                              src={url}
                              alt={name}
                              title={name}
                              className={`icon ${i === 0 ? "icon-rune-keystone" : "icon-rune"}`}
                            />
                          ) : (
                            <span key={i} className="icon-fallback">
                              {name}
                            </span>
                          );
                        })}
                      </div>
                      <div className="rune-col rune-col-secondary">
                        {runes.secondary_rune_names?.map((name, i) => {
                          const url = runeIconUrl(ddragon, name);
                          return url ? (
                            <img
                              key={i}
                              src={url}
                              alt={name}
                              title={name}
                              className="icon icon-rune"
                            />
                          ) : (
                            <span key={i} className="icon-fallback">
                              {name}
                            </span>
                          );
                        })}
                        {runes.stat_mod_names && runes.stat_mod_names.length > 0 && (
                          <div className="rune-shards">
                            {runes.stat_mod_names.map((id, i) => {
                              const url = statModIconUrl(id);
                              const name = statModName(id);
                              return url ? (
                                <img
                                  key={i}
                                  src={url}
                                  alt={name}
                                  title={name}
                                  className="icon icon-shard"
                                />
                              ) : (
                                <span key={i} className="icon-fallback">
                                  {name}
                                </span>
                              );
                            })}
                          </div>
                        )}
                      </div>
                    </div>
                    <span className={`stat conf-${confidenceFor(runes.play ?? 0)}`}>
                      {winRate(runes.win ?? 0, runes.play ?? 0).toFixed(1)}% &middot;{" "}
                      {formatGames(runes.play ?? 0)} games
                    </span>
                  </div>
                )
              ))}

              {activeManualBuild ? (
                <div className="build-row">
                  <span className="build-row-label">SKILL ORDER</span>
                  <SkillGrid order={activeManualBuild.skillOrder} />
                </div>
              ) : (
                skills && (
                  <div className="build-row">
                    <span className="build-row-label">SKILL ORDER</span>
                    <SkillGrid order={extendSkillOrder(skills)} />
                  </div>
                )
              )}

              {strong.length === 0 && weak.length === 0 && analysis.data?.counters_meta?.message ? (
                <p className="counters-empty-note">{analysis.data.counters_meta.message}</p>
              ) : (
              <div className="counters">
                <div className="counter-col">
                  <span className="build-row-label">BEST INTO</span>
                  {strong.map((c, i) => {
                    const url = c.champion_name
                      ? championIconUrl(ddragon, c.champion_name)
                      : undefined;
                    return (
                      <div key={i} className="counter-row counter-good">
                        <span className="counter-champ">
                          {url && (
                            <img src={url} alt={c.champion_name} className="icon icon-counter" />
                          )}
                          {c.champion_name}
                        </span>
                        <span>{((c.my_win_rate ?? 0) * 100).toFixed(1)}%</span>
                      </div>
                    );
                  })}
                </div>
                <div className="counter-col">
                  <span className="build-row-label">WORST INTO</span>
                  {weak.map((c, i) => {
                    const url = c.champion_name
                      ? championIconUrl(ddragon, c.champion_name)
                      : undefined;
                    return (
                      <div key={i} className="counter-row counter-bad">
                        <span className="counter-champ">
                          {url && (
                            <img src={url} alt={c.champion_name} className="icon icon-counter" />
                          )}
                          {c.champion_name}
                        </span>
                        <span>{((c.my_win_rate ?? 0) * 100).toFixed(1)}%</span>
                      </div>
                    );
                  })}
                </div>
              </div>
              )}
            </div>
          </div>

          {gameMode === "aram" && augments.length > 0 && (
            <div className="build-row">
              <div className="augment-header-row">
                <span className="build-row-label">ARAM AUGMENTS</span>
                <input
                  type="text"
                  className="augment-search"
                  placeholder="Type your augment..."
                  value={augmentSearch}
                  onChange={(e) => setAugmentSearch(e.target.value)}
                />
              </div>
              <div className="augment-grid">
                {(() => {
                  const query = augmentSearch.trim().toLowerCase();
                  const visible = augments.filter((a) => a.name?.toLowerCase().includes(query));
                  if (visible.length === 0) {
                    return <p className="stat">No augment matches "{augmentSearch}"</p>;
                  }
                  return visible.map((a) => {
                    const rank = augments.indexOf(a);
                    const hasData = rank < augmentDataCount;
                    const grade = hasData && a.tier !== undefined ? gradeFromTier(a.tier) : null;
                    return (
                      <div key={a.id ?? rank} className="augment-card">
                        <div className="augment-card-head">
                          <span className="augment-name">{a.name}</span>
                          {grade ? (
                            <span
                              className="tierlist-grade"
                              style={{ color: gradeColorVar(grade) }}
                            >
                              {grade}
                            </span>
                          ) : (
                            <span className="stat-sm">No data yet</span>
                          )}
                        </div>
                        {hasData ? (
                          <>
                            <p className="augment-desc">{stripTags(a.desc ?? "")}</p>
                            <span className="stat-sm">
                              Pick {((a.popular ?? 0) * 100).toFixed(0)}%
                            </span>
                          </>
                        ) : (
                          <p className="augment-desc">
                            New augment — OP.GG doesn't have stats for this champion yet.
                          </p>
                        )}
                      </div>
                    );
                  });
                })()}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
