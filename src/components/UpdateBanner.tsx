import { useEffect, useState } from "react";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import "./UpdateBanner.css";

export default function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [status, setStatus] = useState<"idle" | "installing" | "error">("idle");

  useEffect(() => {
    check()
      .then((result) => {
        if (result?.available) setUpdate(result);
      })
      .catch(() => {
        // No network, no releases yet, etc. — silently skip, this is a
        // background check, not something the user needs to see fail.
      });
  }, []);

  if (!update) return null;

  async function install() {
    if (!update) return;
    setStatus("installing");
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch {
      setStatus("error");
    }
  }

  return (
    <div className="update-banner">
      <span>
        New version {update.version} available
        {status === "error" && " — update failed, try again later"}
      </span>
      <button
        type="button"
        className="update-banner-btn"
        disabled={status === "installing"}
        onClick={install}
      >
        {status === "installing" ? "Installing..." : "Update & Restart"}
      </button>
    </div>
  );
}
