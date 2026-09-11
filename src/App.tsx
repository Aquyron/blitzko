import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ChampionLookup from "./components/ChampionLookup";
import TierList from "./components/TierList";
import ChampSelectAssist from "./components/ChampSelectAssist";
import UpdateBanner from "./components/UpdateBanner";
import { loadChampionIdIndex } from "./lib/ddragon";
import "./App.css";

type GameflowState = { phase: string };

type ChampSelectState = {
  active: boolean;
  myChampionId: number | null;
  myChampionLocked: boolean;
  myPosition: string | null;
  enemyChampionIds: number[];
  queueGameMode: string | null;
  myActionId: number | null;
  myActionType: string | null;
  myBanPending: boolean;
  bannedChampionIds: number[];
  myTeamChampionIds: number[];
};

function App() {
  const [gameflow, setGameflow] = useState<GameflowState>({ phase: "None" });
  const [champSelect, setChampSelect] = useState<ChampSelectState>({
    active: false,
    myChampionId: null,
    myChampionLocked: false,
    myPosition: null,
    enemyChampionIds: [],
    queueGameMode: null,
    myActionId: null,
    myActionType: null,
    myBanPending: false,
    bannedChampionIds: [],
    myTeamChampionIds: [],
  });
  const [championIdMap, setChampionIdMap] = useState<Map<number, string> | null>(null);
  const [tab, setTab] = useState<"build" | "tierlist">("build");

  useEffect(() => {
    invoke<GameflowState>("get_gameflow_phase").then(setGameflow);
    invoke<ChampSelectState>("get_champ_select_state").then(setChampSelect);
    loadChampionIdIndex().then(setChampionIdMap);

    const unlistenGameflow = listen<GameflowState>("gameflow-phase", (e) =>
      setGameflow(e.payload)
    );
    const unlistenChampSelect = listen<ChampSelectState>("champ-select-state", (e) =>
      setChampSelect(e.payload)
    );

    return () => {
      unlistenGameflow.then((f) => f());
      unlistenChampSelect.then((f) => f());
    };
  }, []);

  // Auto-switch to the build tab once our own pick is actually locked in —
  // not just hovered, since the pre-ban "pick intent" window also populates
  // myChampionId and would otherwise skip the ban suggestions entirely.
  useEffect(() => {
    if (champSelect.active && champSelect.myChampionLocked) {
      setTab("build");
    }
  }, [champSelect.active, champSelect.myChampionLocked]);

  const autoChampion =
    championIdMap && champSelect.myChampionId
      ? championIdMap.get(champSelect.myChampionId)
      : undefined;
  const autoPosition = champSelect.myPosition ?? undefined;
  const autoGameMode = champSelect.queueGameMode ?? undefined;

  return (
    <main className="app">
      <div className="app-stack">
      <UpdateBanner />
      {champSelect.active && !champSelect.myChampionLocked ? (
        <ChampSelectAssist
          myPosition={champSelect.myPosition}
          myActionId={champSelect.myActionId}
          myActionType={champSelect.myActionType}
          myBanPending={champSelect.myBanPending}
          bannedChampionIds={champSelect.bannedChampionIds}
          myTeamChampionIds={champSelect.myTeamChampionIds}
          enemyChampionIds={champSelect.enemyChampionIds}
        />
      ) : (
        <>
          <div className="tab-switch">
            <button
              className={`tab-btn${tab === "build" ? " active" : ""}`}
              onClick={() => setTab("build")}
            >
              Champion Build
            </button>
            <button
              className={`tab-btn${tab === "tierlist" ? " active" : ""}`}
              onClick={() => setTab("tierlist")}
            >
              Tier List
            </button>
          </div>

          {tab === "build" ? (
            <ChampionLookup
              autoChampion={autoChampion}
              autoPosition={autoPosition}
              autoGameMode={autoGameMode}
              gameflowPhase={gameflow.phase}
            />
          ) : (
            <TierList />
          )}
        </>
      )}
      </div>
    </main>
  );
}

export default App;
