import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface PlatformCapabilities {
  os: string;
  arch: string;
  nativeWindowControls: boolean;
  windowsShell: boolean;
  inAppUpdate: boolean;
}

// Bootstrap layout before the first IPC response to avoid Windows controls flashing on Mac.
export const isMac = /Mac/.test(navigator.platform);
const initial: PlatformCapabilities = {
  os: isMac ? "macos" : "windows", arch: "", nativeWindowControls: isMac,
  windowsShell: !isMac, inAppUpdate: !isMac
};

export function usePlatform() {
  const [platform, setPlatform] = useState(initial);
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let active = true;
    void invoke<PlatformCapabilities>("get_platform_capabilities").then((value) => {
      if (active) setPlatform(value);
    }).catch(() => undefined);
    return () => { active = false; };
  }, []);
  return platform;
}
