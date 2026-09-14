use std::path::PathBuf;

use super::types::LcuConnectionInfo;

/// Candidate lockfile locations to watch, most likely first.
///
/// Confirmed live on macOS (2026-09, Riot Client unified installer): the
/// League Client writes its lockfile straight into the game's install dir,
/// `<League.app>/Contents/LoL/lockfile` — the same `<install
/// dir>/lockfile` convention long documented for Windows. (The per-product
/// `Metadata/<product>.<patchline>/` folder under `/Users/Shared/Riot
/// Games/` only holds Riot Client's own install-status bookkeeping, not the
/// LCU lockfile — confirmed by testing against a real running client.)
pub fn candidate_lockfile_paths() -> Vec<PathBuf> {
    if let Ok(path) = std::env::var("BLITZKO_MOCK_LOCKFILE") {
        return vec![PathBuf::from(path)];
    }

    #[cfg(target_os = "windows")]
    {
        // Confirmed live (2026-09): a real install can sit on any drive
        // (e.g. `D:\Riot Games\League of Legends`), not just the default
        // `C:\` — hardcoding C: alone meant the app silently never
        // connected for anyone who installed elsewhere. Riot's own
        // Riot Client records every product's real install directory in
        // `RiotClientInstalls.json` regardless of drive, so read that
        // first; the hardcoded paths below are only a fallback for the
        // (default-location) case where that file is missing or its shape
        // has changed.
        let mut candidates = read_riot_client_installs();
        // Covers every possible drive letter, not just C/D/E — a friend's
        // install on, say, E: (or any other letter) at the default
        // `<drive>:\Riot Games\League of Legends` path still gets found
        // even if RiotClientInstalls.json is missing/unreadable. Cheap:
        // each candidate is only ever touched by an `exists()` check on its
        // parent dir, so a non-existent drive letter just gets skipped.
        for letter in b'A'..=b'Z' {
            candidates.push(PathBuf::from(format!(
                "{}:\\Riot Games\\League of Legends\\lockfile",
                letter as char
            )));
        }
        candidates
    }

    #[cfg(target_os = "macos")]
    {
        vec![PathBuf::from(
            "/Applications/League of Legends.app/Contents/LoL/lockfile",
        )]
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        vec![]
    }
}

/// Reads `%ProgramData%\Riot Games\RiotClientInstalls.json`, which Riot
/// Client itself maintains with every installed product's real install
/// directory — the only reliable way to find a League install that isn't
/// on the default drive. **Unverified live against this exact file** — the
/// two known community-documented shapes are handled defensively:
/// - older/simple: a top-level string value keyed by product id, e.g.
///   `"league_of_legends.live": "D:\\Riot Games\\League of Legends"`
/// - newer: an `"associated_client"` object whose *keys* are full paths to
///   `LeagueClient.exe`, e.g.
///   `"D:\\Riot Games\\League of Legends\\LeagueClient.exe": "league_of_legends.live"`
#[cfg(target_os = "windows")]
fn read_riot_client_installs() -> Vec<PathBuf> {
    let program_data =
        std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let installs_path = PathBuf::from(program_data)
        .join("Riot Games")
        .join("RiotClientInstalls.json");

    let Ok(content) = std::fs::read_to_string(&installs_path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Vec::new();
    };

    let mut dirs: Vec<PathBuf> = Vec::new();

    // Older shape: top-level "league_of_legends*" keys pointing straight at
    // the install directory.
    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            if key.starts_with("league_of_legends") {
                if let Some(dir) = val.as_str() {
                    dirs.push(PathBuf::from(dir));
                }
            }
        }
    }

    // Newer shape: "associated_client" keyed by the full exe path.
    if let Some(assoc) = value.get("associated_client").and_then(|v| v.as_object()) {
        for (exe_path, product) in assoc {
            let is_league = product
                .as_str()
                .map(|p| p.starts_with("league_of_legends"))
                .unwrap_or(false);
            if is_league {
                if let Some(dir) = PathBuf::from(exe_path).parent() {
                    dirs.push(dir.to_path_buf());
                }
            }
        }
    }

    dirs.into_iter().map(|dir| dir.join("lockfile")).collect()
}

/// Parses the lockfile's `name:pid:port:password:protocol` line.
pub fn parse_lockfile(path: &std::path::Path) -> Option<LcuConnectionInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let parts: Vec<&str> = content.trim().split(':').collect();
    if parts.len() != 5 {
        return None;
    }
    Some(LcuConnectionInfo {
        pid: parts[1].parse().ok()?,
        port: parts[2].parse().ok()?,
        password: parts[3].to_string(),
        protocol: parts[4].to_string(),
    })
}
