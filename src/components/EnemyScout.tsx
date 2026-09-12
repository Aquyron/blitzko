import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DdragonIndex, championIconUrl, loadDdragonIndex } from "../lib/ddragon";
import { rankIconUrl } from "../lib/riotAssets";
import "./EnemyScout.css";

type EnemyInfo = {
  championName: string;
  gameName: string;
  tagLine: string;
};

type RankInfo = {
  tier: string;
  division: string;
  lp: number;
  win: number;
  lose: number;
} | null;

type LeagueStatsEntry = {
  game_type?: string;
  tier_info?: { tier?: string; division?: string; lp?: number };
  win?: number;
  lose?: number;
};

type SummonerProfileResponse = {
  data?: { summoner?: { league_stats?: LeagueStatsEntry[] } };
};

function pickSoloQueue(stats: LeagueStatsEntry[]): LeagueStatsEntry | undefined {
  return (
    stats.find((s) => s.game_type?.toUpperCase().includes("SOLO")) ?? stats[0]
  );
}

export default function EnemyScout() {
  const [ddragon, setDdragon] = useState<DdragonIndex | null>(null);
  const [enemies, setEnemies] = useState<EnemyInfo[] | null>(null);
  const [ranks, setRanks] = useState<Record<string, RankInfo>>({});
  const [error, setError] = useState<string | null>(null);
  const fetchedRanks = useRef(new Set<string>());

  useEffect(() => {
    loadDdragonIndex().then(setDdragon);
  }, []);

  // The live-client-data port only starts responding a few seconds after
  // the match actually loads, so the first attempt or two right after
  // gameflow flips to InProgress can fail — retry instead of giving up.
  useEffect(() => {
    let cancelled = false;
    let attempts = 0;

    async function tryFetch() {
      try {
        const result = await invoke<EnemyInfo[]>("get_live_game_enemies");
        if (cancelled) return;
        if (result.length > 0) {
          setEnemies(result);
          setError(null);
          return;
        }
      } catch {
        // not ready yet, or not actually in a live game — keep retrying
      }
      attempts += 1;
      if (attempts < 15 && !cancelled) {
        setTimeout(tryFetch, 2000);
      } else if (!cancelled) {
        setError("Could not read live game data.");
      }
    }

    tryFetch();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!enemies) return;
    for (const enemy of enemies) {
      const key = `${enemy.gameName}#${enemy.tagLine}`;
      if (fetchedRanks.current.has(key)) continue;
      fetchedRanks.current.add(key);
      invoke<SummonerProfileResponse>("get_enemy_rank", {
        gameName: enemy.gameName,
        tagLine: enemy.tagLine,
      })
        .then((res) => {
          const stats = res.data?.summoner?.league_stats ?? [];
          const solo = pickSoloQueue(stats);
          const tierInfo = solo?.tier_info;
          if (!tierInfo?.tier) {
            setRanks((prev) => ({ ...prev, [key]: null }));
            return;
          }
          setRanks((prev) => ({
            ...prev,
            [key]: {
              tier: tierInfo.tier!,
              division: tierInfo.division ?? "",
              lp: tierInfo.lp ?? 0,
              win: solo?.win ?? 0,
              lose: solo?.lose ?? 0,
            },
          }));
        })
        .catch(() => {
          setRanks((prev) => ({ ...prev, [key]: null }));
        });
    }
  }, [enemies]);

  if (!ddragon) return null;

  return (
    <div className="enemy-scout">
      <div className="enemy-scout-label">Enemy Team</div>
      {error && <p className="error-text">{error}</p>}
      {!enemies && !error && <p className="enemy-scout-hint">Loading live game data...</p>}
      {enemies && (
        <div className="enemy-scout-list">
          {enemies.map((enemy) => {
            const icon = championIconUrl(ddragon, enemy.championName);
            const key = `${enemy.gameName}#${enemy.tagLine}`;
            const rank = ranks[key];
            const rankIcon = rank ? rankIconUrl(rank.tier.toLowerCase()) : undefined;
            const games = rank ? rank.win + rank.lose : 0;
            const wr = games > 0 ? ((rank!.win / games) * 100).toFixed(0) : null;
            return (
              <div key={key} className="enemy-scout-row">
                {icon && <img src={icon} alt={enemy.championName} className="enemy-scout-icon" />}
                <span className="enemy-scout-name">
                  {enemy.gameName}
                  <span className="enemy-scout-tag">#{enemy.tagLine}</span>
                </span>
                <span className="enemy-scout-rank">
                  {rank === undefined ? (
                    "..."
                  ) : rank === null ? (
                    "Unranked"
                  ) : (
                    <>
                      {rankIcon && <img src={rankIcon} alt={rank.tier} className="enemy-scout-rank-icon" />}
                      {rank.tier} {rank.division} {rank.lp} LP
                      {wr && <span className="enemy-scout-wr"> · {wr}% ({games})</span>}
                    </>
                  )}
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
