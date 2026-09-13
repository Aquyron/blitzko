import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  DdragonIndex,
  championIconUrl,
  loadDdragonIndex,
  runeIconUrl,
  spellIconUrlByName,
} from "../lib/ddragon";
import { rankIconUrl } from "../lib/riotAssets";
import "./EnemyScout.css";

type EnemyInfo = {
  championName: string;
  gameName: string;
  tagLine: string;
  position: string;
  spell1Name: string;
  spell2Name: string;
  keystoneName: string;
  primaryTreeName: string;
  secondaryTreeName: string;
};

type RankInfo = {
  tier: string;
  division: string;
  lp: number;
  win: number;
  lose: number;
} | null;

type LaneInfo = {
  mainPosition: string | null;
  isMainRole: boolean;
  laneGames: number;
  laneWins: number;
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

type Props = {
  label: string;
  fetchCommand: "get_live_game_enemies" | "get_live_game_allies";
  variant: "enemy" | "ally";
};

export default function EnemyScout({ label, fetchCommand, variant }: Props) {
  const [ddragon, setDdragon] = useState<DdragonIndex | null>(null);
  const [enemies, setEnemies] = useState<EnemyInfo[] | null>(null);
  const [ranks, setRanks] = useState<Record<string, RankInfo>>({});
  const [lanes, setLanes] = useState<Record<string, LaneInfo>>({});
  const [error, setError] = useState<string | null>(null);
  const fetchedExtras = useRef(new Set<string>());
  const fetchedLanes = useRef(new Set<string>());

  useEffect(() => {
    loadDdragonIndex().then(setDdragon);
  }, []);

  // The live-client-data port only starts responding a few seconds after
  // the match actually loads, so the first attempt or two right after
  // gameflow flips to GameStart/InProgress can fail — retry instead of
  // giving up. Position specifically can also still read "NONE" on that
  // first successful response (it settles in a moment later), so keep
  // refreshing periodically rather than treating the first response as
  // final — otherwise a player whose position wasn't resolved yet never
  // gets a main-role/lane comparison for the rest of the game.
  useEffect(() => {
    let cancelled = false;
    let attempts = 0;
    let refreshTimer: ReturnType<typeof setTimeout> | undefined;

    async function poll() {
      try {
        const result = await invoke<EnemyInfo[]>(fetchCommand);
        if (cancelled) return;
        if (result.length > 0) {
          setEnemies(result);
          setError(null);
          const stillResolving = result.some((e) => e.position === "NONE");
          if (stillResolving && attempts < 20) {
            attempts += 1;
            refreshTimer = setTimeout(poll, 15000);
          }
          return;
        }
      } catch {
        // not ready yet, or not actually in a live game — keep retrying
      }
      attempts += 1;
      if (attempts < 15 && !cancelled) {
        refreshTimer = setTimeout(poll, 2000);
      } else if (!cancelled) {
        setError("Could not read live game data.");
      }
    }

    poll();
    return () => {
      cancelled = true;
      if (refreshTimer) clearTimeout(refreshTimer);
    };
  }, [fetchCommand]);

  useEffect(() => {
    if (!enemies) return;
    for (const enemy of enemies) {
      const key = `${enemy.gameName}#${enemy.tagLine}`;

      if (!fetchedExtras.current.has(key)) {
        fetchedExtras.current.add(key);
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

      // Keyed by position too — position can still be "NONE" on the first
      // successful live-game read and only resolve a bit later, so a
      // change in position (not just a first sighting of this player)
      // should trigger a fresh lane-stats lookup.
      if (enemy.position && enemy.position !== "NONE") {
        const laneKey = `${key}:${enemy.position}`;
        if (!fetchedLanes.current.has(laneKey)) {
          fetchedLanes.current.add(laneKey);
          invoke<LaneInfo>("get_enemy_lane_stats", {
            gameName: enemy.gameName,
            tagLine: enemy.tagLine,
            currentPosition: enemy.position,
          })
            .then((res) => setLanes((prev) => ({ ...prev, [key]: res })))
            .catch(() => setLanes((prev) => ({ ...prev, [key]: null })));
        }
      }
    }
  }, [enemies]);

  if (!ddragon) return null;

  return (
    <div className="enemy-scout">
      <div className={`enemy-scout-label ${variant}`}>{label}</div>
      {error && <p className="error-text">{error}</p>}
      {!enemies && !error && <p className="enemy-scout-hint">Loading live game data...</p>}
      {enemies && (
        <div className="enemy-scout-list">
          {enemies.map((enemy) => {
            const icon = championIconUrl(ddragon, enemy.championName);
            const key = `${enemy.gameName}#${enemy.tagLine}`;
            const rank = ranks[key];
            const lane = lanes[key];
            const rankIcon = rank ? rankIconUrl(rank.tier.toLowerCase()) : undefined;
            const games = rank ? rank.win + rank.lose : 0;
            const wr = games > 0 ? ((rank!.win / games) * 100).toFixed(0) : null;

            const spell1Icon = enemy.spell1Name
              ? spellIconUrlByName(ddragon, enemy.spell1Name)
              : undefined;
            const spell2Icon = enemy.spell2Name
              ? spellIconUrlByName(ddragon, enemy.spell2Name)
              : undefined;
            const keystoneIcon = enemy.keystoneName
              ? runeIconUrl(ddragon, enemy.keystoneName)
              : undefined;
            const secondaryTreeIcon = enemy.secondaryTreeName
              ? runeIconUrl(ddragon, enemy.secondaryTreeName)
              : undefined;

            const laneGames = lane ? lane.laneGames : 0;
            const laneWr =
              lane && laneGames > 0 ? ((lane.laneWins / laneGames) * 100).toFixed(0) : null;

            return (
              <div key={key} className="enemy-scout-row">
                <div className="enemy-scout-main">
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
                        {rankIcon && (
                          <img src={rankIcon} alt={rank.tier} className="enemy-scout-rank-icon" />
                        )}
                        {rank.tier} {rank.division} {rank.lp} LP
                        {wr && (
                          <span className="enemy-scout-wr">
                            {" "}
                            · {wr}% ({games})
                          </span>
                        )}
                      </>
                    )}
                  </span>
                </div>

                <div className="enemy-scout-details">
                  <div className="enemy-scout-loadout">
                    {spell1Icon && <img src={spell1Icon} alt="" className="enemy-scout-spell" />}
                    {spell2Icon && <img src={spell2Icon} alt="" className="enemy-scout-spell" />}
                    {keystoneIcon && (
                      <img src={keystoneIcon} alt="" className="enemy-scout-rune" />
                    )}
                    {secondaryTreeIcon && (
                      <img src={secondaryTreeIcon} alt="" className="enemy-scout-rune-tree" />
                    )}
                  </div>

                  {lane === undefined ? null : lane === null ? null : (
                    <div className="enemy-scout-lane">
                      {laneWr && (
                        <span className="enemy-scout-lane-wr">
                          {laneWr}% WR on {enemy.position.toLowerCase()} ({laneGames})
                        </span>
                      )}
                      <span
                        className={`enemy-scout-lane-tag${
                          lane.isMainRole ? " correct" : " autofill"
                        }`}
                      >
                        {lane.isMainRole ? "Correct lane" : "Autofilled?"}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
