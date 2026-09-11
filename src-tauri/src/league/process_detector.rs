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
        vec![PathBuf::from(r"C:\Riot Games\League of Legends\lockfile")]
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
