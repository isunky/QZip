import { useCallback, useEffect, useRef, useState } from "react";
import type { DownloadedUpdate, UpdateCheckResult, UpdateDownloadProgress } from "../../contracts/settings";
import { settingsClient } from "../../lib/settingsClient";

interface UpdateIssue { code: string; message: string }

function issue(reason: unknown): UpdateIssue {
  if (typeof reason === "object" && reason !== null && "code" in reason && "message" in reason) {
    return { code: String(reason.code), message: String(reason.message) };
  }
  return { code: "UPDATE_UNKNOWN", message: typeof reason === "string" ? reason : "更新操作失败，请重试。" };
}

export function useAppUpdates(checkOnStartup: boolean) {
  const [result, setResult] = useState<UpdateCheckResult | null>(null);
  const [phase, setPhase] = useState<"idle" | "checking" | "downloading" | "verifying" | "ready" | "installing">("idle");
  const [progress, setProgress] = useState<UpdateDownloadProgress | null>(null);
  const [downloaded, setDownloaded] = useState<DownloadedUpdate | null>(null);
  const [error, setError] = useState<UpdateIssue | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const busy = useRef(false);
  const startupChecked = useRef(false);

  const check = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setPhase("checking");
    setError(null);
    setCancelled(false);
    try {
      const next = await settingsClient.checkForUpdates();
      setResult(next);
      setDownloaded(null);
    } catch (reason) { setError(issue(reason)); }
    finally { busy.current = false; setPhase("idle"); }
  }, []);

  useEffect(() => {
    if (!settingsClient.isTauri || !checkOnStartup || startupChecked.current) return;
    const timer = window.setTimeout(() => {
      startupChecked.current = true;
      void check();
    }, 800);
    return () => window.clearTimeout(timer);
  }, [checkOnStartup, check]);

  async function download() {
    if (busy.current || !result?.releaseTag || !result.downloadAvailable) return;
    busy.current = true;
    setError(null);
    setCancelled(false);
    setDownloaded(null);
    setProgress(null);
    setPhase("downloading");
    try {
      const ready = await settingsClient.downloadUpdate(result.releaseTag, (next) => { setProgress(next); setPhase(next.phase); });
      setDownloaded(ready);
      setPhase("ready");
    } catch (reason) {
      const failure = issue(reason);
      if (failure.code === "UPDATE_CANCELLED") setCancelled(true);
      else setError(failure);
      setPhase("idle");
    } finally { busy.current = false; setCancelling(false); }
  }

  async function cancel() {
    if (cancelling) return;
    setCancelling(true);
    try { await settingsClient.cancelDownload(); }
    catch (reason) { setError(issue(reason)); setCancelling(false); }
  }

  async function install() {
    if (busy.current || !downloaded) return;
    busy.current = true;
    setPhase("installing");
    setError(null);
    try { await settingsClient.installUpdate(downloaded.token); }
    catch (reason) {
      const failure = issue(reason);
      setError(failure);
      if (failure.code === "UPDATE_CHECKSUM_MISMATCH" || failure.code === "UPDATE_NOT_READY") { setDownloaded(null); setPhase("idle"); }
      else setPhase("ready");
    } finally { busy.current = false; }
  }

  async function openRelease(url: string) {
    try { await settingsClient.openUpdateLink(url); }
    catch (reason) { setError(issue(reason)); }
  }

  return { result, phase, progress, downloaded, error, cancelled, cancelling, check, download, cancel, install, openRelease };
}

export type AppUpdates = ReturnType<typeof useAppUpdates>;
