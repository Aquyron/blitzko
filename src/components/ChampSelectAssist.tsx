import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DdragonIndex, championIconUrl, loadDdragonIndex, opggChampionKey } from "../lib/ddragon";
import { gradeColorVar, gradeFromTier } from "../lib/recommendation";
import "./ChampSelectAssist.css";

type LaneEntry = {
  champion: string;
  win_rate: number;
  tier: number;
};

type LaneMetaResponse = {
  data?: {
    positions?: Record<string, LaneEntry[]>;
  };
};

type CounterAnalysisResponse = {
  data?: {
    weak_counters?: { champion_name?: string }[];
  };
};

type Props = {
  myPosition: string | null;
  myActionId: number | null;
  myActionType: string | null;
  myBanPending: boolean;
  bannedChampionIds: number[];
  myTeamChampionIds: number[];
  enemyChampionIds: number[];
  enemyTeamChampions: { championId: number; position: string }[];
};

type Suggestion = {
  champion: string;
  championId: number;
  tier: number;
  countersEnemies: string[];
};

export default function ChampSelectAssist({
  myPosition,
  myActionId,
  myActionType,
  myBanPending,
  bannedChampionIds,
  myTeamChampionIds,
  enemyChampionIds,
  enemyTeamChampions,
}: Props) {
  const [ddragon, setDdragon] = useState<DdragonIndex | null>(null);
  const [entries, setEntries] = useState<LaneEntry[]>([]);
  const [submitting, setSubmitting] = useState<number | null>(null);
  // champion name -> names of already-picked enemies it counters well,
  // per OP.GG's own weak_counters for each enemy (i.e. "champions that beat
  // this enemy"). Only meaningful once at least one enemy has locked a
  // champion — empty otherwise, which naturally falls back to plain tier
  // order below.
  const [counterInfo, setCounterInfo] = useState<Record<string, string[]>>({});
  const fetchedEnemyKeys = useRef(new Set<string>());
  // The LCU session can momentarily report no position/spells right as champ
  // select is wrapping up (about to unmount anyway) — stick with the last
  // real reading instead of flashing back to "detecting" for one frame.
  const [lastKnownPosition, setLastKnownPosition] = useState<string | null>(null);
  const position = myPosition ?? lastKnownPosition;

  useEffect(() => {
    loadDdragonIndex().then(setDdragon);
  }, []);

  useEffect(() => {
    if (myPosition) setLastKnownPosition(myPosition);
  }, [myPosition]);

  useEffect(() => {
    if (!position) {
      setEntries([]);
      return;
    }
    let cancelled = false;
    invoke<LaneMetaResponse>("get_lane_tier_list", { position }).then((res) => {
      if (cancelled) return;
      const list = res.data?.positions?.[position] ?? [];
      setEntries([...list].sort((a, b) => a.tier - b.tier || b.win_rate - a.win_rate));
    });
    return () => {
      cancelled = true;
    };
  }, [position]);

  const idToName = useMemo(() => {
    const m = new Map<number, string>();
    if (!ddragon) return m;
    for (const [name, c] of ddragon.championByName) m.set(Number(c.key), name);
    return m;
  }, [ddragon]);

  // Once an enemy has actually locked a champion, pull who counters *them*
  // (OP.GG's own weak_counters for that enemy) so a great counter pick can
  // be surfaced ahead of the plain tier list — one request per enemy
  // champion, not per suggestion, and cached both here and server-side so
  // re-renders don't refetch.
  useEffect(() => {
    if (!ddragon) return;
    for (const enemy of enemyTeamChampions) {
      const name = idToName.get(enemy.championId);
      if (!name) continue;
      const key = `${name}:${enemy.position}`;
      if (fetchedEnemyKeys.current.has(key)) continue;
      fetchedEnemyKeys.current.add(key);
      const opggChampion = opggChampionKey(ddragon, name);
      invoke<CounterAnalysisResponse>("get_champion_build", {
        champion: opggChampion,
        position: enemy.position,
        tier: "emerald_plus",
        gameMode: "ranked",
      })
        .then((res) => {
          const counters = (res.data?.weak_counters ?? [])
            .map((c) => c.champion_name)
            .filter((n): n is string => Boolean(n));
          if (counters.length === 0) return;
          setCounterInfo((prev) => {
            const next = { ...prev };
            for (const counterName of counters) {
              next[counterName] = [...(next[counterName] ?? []), name];
            }
            return next;
          });
        })
        .catch(() => {});
    }
  }, [ddragon, enemyTeamChampions, idToName]);

  if (!ddragon) return null;

  if (!position || entries.length === 0) {
    return (
      <div className="csa">
        <p className="csa-hint">
          {position
            ? "Loading meta data for your role..."
            : "Detecting your role (position or summoner spells)..."}
        </p>
      </div>
    );
  }

  const toSuggestion = (e: LaneEntry): Suggestion | null => {
    const key = ddragon.championByName.get(e.champion)?.key;
    if (!key) return null;
    return {
      champion: e.champion,
      championId: Number(key),
      tier: e.tier,
      countersEnemies: counterInfo[e.champion] ?? [],
    };
  };

  const banned = new Set(bannedChampionIds);
  const mine = new Set(myTeamChampionIds);
  const enemy = new Set(enemyChampionIds);

  const suggestions = entries.map(toSuggestion).filter((s): s is Suggestion => s !== null);

  // Once an enemy has actually locked a champion, a real counter to them
  // beats a generically-strong pick — bubble those to the front of the
  // pick list. Stable sort keeps the existing tier order within each group,
  // and this is a no-op before anyone's picked anything (countersEnemies is
  // empty for everyone), so first-pick behavior is unchanged.
  const pickRanked = [...suggestions].sort(
    (a, b) => (b.countersEnemies.length > 0 ? 1 : 0) - (a.countersEnemies.length > 0 ? 1 : 0)
  );

  const suggestedBans = suggestions
    .filter((s) => !banned.has(s.championId) && !mine.has(s.championId) && !enemy.has(s.championId))
    .slice(0, 3);

  const suggestedPicks = pickRanked
    .filter((s) => !banned.has(s.championId) && !mine.has(s.championId))
    .slice(0, 6);

  const canBan = myActionType === "ban" && myActionId !== null;
  const canPick = myActionType === "pick" && myActionId !== null;

  async function submit(championId: number) {
    if (myActionId === null) return;
    setSubmitting(championId);
    try {
      await invoke("submit_champ_select_action", {
        actionId: myActionId,
        championId,
      });
    } catch (e) {
      console.error("champ select action failed", e);
    } finally {
      setSubmitting(null);
    }
  }

  const renderGrid = (list: Suggestion[], enabled: boolean, disabledHint: string) => (
    <div className="csa-grid">
      {list.map((s) => {
        const icon = championIconUrl(ddragon, s.champion);
        const grade = gradeFromTier(s.tier);
        return (
          <button
            key={s.championId}
            type="button"
            className="csa-item"
            disabled={!enabled || submitting !== null}
            title={enabled ? s.champion : `${s.champion} — ${disabledHint}`}
            onClick={() => submit(s.championId)}
          >
            {icon && <img src={icon} alt={s.champion} className="csa-icon" />}
            <span className="csa-name">{s.champion}</span>
            <span className="csa-grade" style={{ color: gradeColorVar(grade) }}>
              {grade}
            </span>
            {s.countersEnemies.length > 0 && (
              <span className="csa-counter-tag">Counters {s.countersEnemies.join(", ")}</span>
            )}
          </button>
        );
      })}
    </div>
  );

  return (
    <div className="csa">
      {myBanPending && (
        <div className="csa-section">
          <div className="csa-label">
            <span>Suggested Bans</span>
            <span className="csa-role-tag">{myPosition}</span>
            {!canBan && <span className="csa-wait">waiting for your ban turn</span>}
          </div>
          {renderGrid(suggestedBans, canBan, "not your ban turn yet")}
        </div>
      )}

      <div className="csa-section">
        <div className="csa-label">
          <span>Suggested Picks</span>
          <span className="csa-role-tag">{myPosition}</span>
          {!canPick && <span className="csa-wait">waiting for your pick turn</span>}
        </div>
        {renderGrid(suggestedPicks, canPick, "not your pick turn yet")}
      </div>
    </div>
  );
}
