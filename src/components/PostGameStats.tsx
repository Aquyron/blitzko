import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DdragonIndex, championIconUrl, loadDdragonIndex } from "../lib/ddragon";
import "./PostGameStats.css";

type PostGamePlayer = {
  summonerName: string;
  championId: number;
  isLocalPlayer: boolean;
  win: boolean;
  kills: number;
  deaths: number;
  assists: number;
  damageDealt: number;
  goldEarned: number;
  cs: number;
  visionScore: number;
  mvpScore: number;
};

type PostGameState = {
  players: PostGamePlayer[];
  localPlayerWin: boolean;
};

function formatNumber(n: number): string {
  return n.toLocaleString();
}

export default function PostGameStats() {
  const [ddragon, setDdragon] = useState<DdragonIndex | null>(null);
  const [stats, setStats] = useState<PostGameState | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadDdragonIndex().then(setDdragon);
  }, []);

  const idToName = useRef<Map<number, string>>(new Map());
  useEffect(() => {
    if (!ddragon) return;
    for (const [name, c] of ddragon.championByName) idToName.current.set(Number(c.key), name);
  }, [ddragon]);

  // The LCU only populates this endpoint for a short window right after a
  // match ends — the very first attempt or two right as the phase flips to
  // WaitingForStats can still 404, so retry instead of giving up.
  useEffect(() => {
    let cancelled = false;
    let attempts = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;

    async function poll() {
      try {
        const result = await invoke<PostGameState>("get_post_game_stats");
        if (cancelled) return;
        if (result.players.length > 0) {
          setStats(result);
          setError(null);
          return;
        }
      } catch {
        // not ready yet — keep retrying
      }
      attempts += 1;
      if (attempts < 15 && !cancelled) {
        timer = setTimeout(poll, 2000);
      } else if (!cancelled) {
        setError("Could not read post-game stats.");
      }
    }

    poll();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, []);

  if (!ddragon) return null;

  return (
    <div className="pgs">
      <div className={`pgs-result ${stats?.localPlayerWin ? "win" : "loss"}`}>
        {!stats ? "Match Summary" : stats.localPlayerWin ? "Victory" : "Defeat"}
      </div>
      {error && <p className="pgs-hint">{error}</p>}
      {!stats && !error && <p className="pgs-hint">Loading post-game stats...</p>}
      {stats && (
        <div className="pgs-list">
          {stats.players.map((p, i) => {
            const championName = idToName.current.get(p.championId);
            const icon = championName ? championIconUrl(ddragon, championName) : undefined;
            const isMvp = i === 0;
            return (
              <div
                key={`${p.summonerName}-${i}`}
                className={`pgs-row${p.isLocalPlayer ? " me" : ""}${p.win ? " win" : " loss"}`}
              >
                <span className="pgs-rank">{isMvp ? "MVP" : `#${i + 1}`}</span>
                {icon && <img src={icon} alt={championName} className="pgs-icon" />}
                <span className="pgs-name">{p.summonerName}</span>
                <span className="pgs-kda">
                  {p.kills} / {p.deaths} / {p.assists}
                </span>
                <span className="pgs-stat">{formatNumber(p.damageDealt)} dmg</span>
                <span className="pgs-stat">{formatNumber(p.goldEarned)} gold</span>
                <span className="pgs-stat">{p.cs} CS</span>
                <span className="pgs-stat">{p.visionScore} vis</span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
