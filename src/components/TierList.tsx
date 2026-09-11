import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DdragonIndex, championIconUrl, loadDdragonIndex } from "../lib/ddragon";
import { formatGames, gradeFromTier, gradeColorVar } from "../lib/recommendation";
import { RolePicker } from "./RolePicker";
import "./TierList.css";

const POSITIONS = ["top", "jungle", "mid", "adc", "support"];

type LaneEntry = {
  champion: string;
  play: number;
  win_rate: number;
  pick_rate: number;
  ban_rate: number;
  tier: number;
};

type LaneMetaResponse = {
  data?: {
    positions?: Record<string, LaneEntry[]>;
  };
};

export default function TierList() {
  const [ddragon, setDdragon] = useState<DdragonIndex | null>(null);
  const [position, setPosition] = useState("jungle");
  const [entries, setEntries] = useState<LaneEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadDdragonIndex().then(setDdragon);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<LaneMetaResponse>("get_lane_tier_list", { position })
      .then((res) => {
        if (cancelled) return;
        const list = res.data?.positions?.[position] ?? [];
        setEntries(
          [...list].sort((a, b) => a.tier - b.tier || b.win_rate - a.win_rate)
        );
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [position]);

  return (
    <div className="tierlist">
      <div className="tierlist-controls">
        <RolePicker options={POSITIONS} value={position} onChange={setPosition} />
        {loading && <span className="stat loading-indicator">Loading...</span>}
      </div>
      {error && <p className="error-text">{error}</p>}

      {ddragon && entries.length > 0 && (
        <div className="tierlist-table">
          <div className="tierlist-row tierlist-header">
            <span>#</span>
            <span>Champion</span>
            <span>Grade</span>
            <span>Win Rate</span>
            <span>Pick Rate</span>
            <span>Ban Rate</span>
            <span>Games</span>
          </div>
          {entries.map((e, i) => {
            const icon = championIconUrl(ddragon, e.champion);
            const wrPct = e.win_rate * 100;
            const grade = gradeFromTier(e.tier);
            return (
              <div key={e.champion} className="tierlist-row">
                <span className="tierlist-rank">{i + 1}</span>
                <span className="tierlist-champ">
                  {icon && <img src={icon} alt={e.champion} className="icon icon-tierlist" />}
                  {e.champion}
                </span>
                <span className="tierlist-grade" style={{ color: gradeColorVar(grade) }}>
                  {grade}
                </span>
                <span>{wrPct.toFixed(1)}%</span>
                <span>{(e.pick_rate * 100).toFixed(1)}%</span>
                <span>{(e.ban_rate * 100).toFixed(1)}%</span>
                <span>{formatGames(e.play)}</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
