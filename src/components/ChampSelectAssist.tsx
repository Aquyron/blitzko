import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DdragonIndex, championIconUrl, loadDdragonIndex } from "../lib/ddragon";
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

type Props = {
  myPosition: string | null;
  myActionId: number | null;
  myActionType: string | null;
  myBanPending: boolean;
  bannedChampionIds: number[];
  myTeamChampionIds: number[];
  enemyChampionIds: number[];
};

type Suggestion = {
  champion: string;
  championId: number;
  tier: number;
};

export default function ChampSelectAssist({
  myPosition,
  myActionId,
  myActionType,
  myBanPending,
  bannedChampionIds,
  myTeamChampionIds,
  enemyChampionIds,
}: Props) {
  const [ddragon, setDdragon] = useState<DdragonIndex | null>(null);
  const [entries, setEntries] = useState<LaneEntry[]>([]);
  const [submitting, setSubmitting] = useState<number | null>(null);
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
    return { champion: e.champion, championId: Number(key), tier: e.tier };
  };

  const banned = new Set(bannedChampionIds);
  const mine = new Set(myTeamChampionIds);
  const enemy = new Set(enemyChampionIds);

  const suggestions = entries.map(toSuggestion).filter((s): s is Suggestion => s !== null);

  const suggestedBans = suggestions
    .filter((s) => !banned.has(s.championId) && !mine.has(s.championId) && !enemy.has(s.championId))
    .slice(0, 3);

  const suggestedPicks = suggestions
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
