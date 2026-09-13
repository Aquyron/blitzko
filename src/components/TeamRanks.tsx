import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { rankIconUrl } from "../lib/riotAssets";
import "./TeamRanks.css";

type Member = { summonerId: number; position: string };

type LaneStatsInfo = {
  mainPosition: string | null;
  isMainRole: boolean;
  laneGames: number;
  laneWins: number;
};

type LeagueStatsEntry = {
  game_type?: string;
  tier_info?: { tier?: string; division?: string; lp?: number };
  win?: number;
  lose?: number;
};

type TeammateRankInfo = {
  gameName: string;
  tagLine: string;
  rank: { data?: { summoner?: { league_stats?: LeagueStatsEntry[] } } } | null;
  lane: LaneStatsInfo;
};

function pickSoloQueue(stats: LeagueStatsEntry[]): LeagueStatsEntry | undefined {
  return stats.find((s) => s.game_type?.toUpperCase().includes("SOLO")) ?? stats[0];
}

export default function TeamRanks({ members }: { members: Member[] }) {
  const [info, setInfo] = useState<Record<number, TeammateRankInfo | null>>({});
  const fetched = useRef(new Set<string>());

  useEffect(() => {
    for (const member of members) {
      const key = `${member.summonerId}:${member.position}`;
      if (fetched.current.has(key)) continue;
      fetched.current.add(key);
      invoke<TeammateRankInfo>("get_teammate_rank", {
        summonerId: member.summonerId,
        position: member.position,
      })
        .then((res) => setInfo((prev) => ({ ...prev, [member.summonerId]: res })))
        .catch(() => setInfo((prev) => ({ ...prev, [member.summonerId]: null })));
    }
  }, [members]);

  if (members.length === 0) return null;

  return (
    <div className="team-ranks">
      <div className="team-ranks-label">My Team</div>
      <div className="team-ranks-list">
        {members.map((member) => {
          const data = info[member.summonerId];
          const stats = data?.rank?.data?.summoner?.league_stats ?? [];
          const solo = pickSoloQueue(stats);
          const tierInfo = solo?.tier_info;
          const rankIcon = tierInfo?.tier ? rankIconUrl(tierInfo.tier.toLowerCase()) : undefined;
          const games = (solo?.win ?? 0) + (solo?.lose ?? 0);
          const wr = games > 0 ? (((solo?.win ?? 0) / games) * 100).toFixed(0) : null;
          const lane = data?.lane;
          const laneGames = lane?.laneGames ?? 0;
          const laneWr =
            lane && laneGames > 0 ? ((lane.laneWins / laneGames) * 100).toFixed(0) : null;

          return (
            <div key={member.summonerId} className="team-ranks-row">
              <span className="team-ranks-name">
                {data === undefined ? "Loading..." : data?.gameName ?? "Unknown"}
                {data?.tagLine && <span className="team-ranks-tag">#{data.tagLine}</span>}
              </span>
              <span className="team-ranks-rank">
                {tierInfo?.tier ? (
                  <>
                    {rankIcon && (
                      <img src={rankIcon} alt={tierInfo.tier} className="team-ranks-rank-icon" />
                    )}
                    {tierInfo.tier} {tierInfo.division} {tierInfo.lp} LP
                    {wr && (
                      <span className="team-ranks-wr">
                        {" "}
                        · {wr}% ({games})
                      </span>
                    )}
                  </>
                ) : data === undefined ? (
                  "..."
                ) : (
                  "Unranked"
                )}
              </span>
              {lane && laneGames > 0 && (
                <span className="team-ranks-lane">
                  {laneWr && `${laneWr}% WR (${laneGames}) · `}
                  <span
                    className={`team-ranks-lane-tag${lane.isMainRole ? " correct" : " autofill"}`}
                  >
                    {lane.isMainRole ? "Correct lane" : "Autofilled?"}
                  </span>
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
