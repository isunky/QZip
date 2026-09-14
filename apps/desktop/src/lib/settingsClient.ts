import { Channel, invoke } from "@tauri-apps/api/core";
import type { AppSettings, AppSettingsPatch, IntegrationStatus, UpdateCheckResult, UpdateDownloadProgress, DownloadedUpdate } from "../contracts/settings";
import { defaultAppSettings } from "../contracts/settings";
import desktopPackage from "../../package.json";

const isTauri = "__TAURI_INTERNALS__" in window;
let previewSettings: AppSettings = { ...defaultAppSettings };
async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(name, args);
}

export const settingsClient = {
  isTauri,
  get: () => isTauri ? command<AppSettings>("get_app_settings") : Promise.resolve(previewSettings),
  update: (patch: AppSettingsPatch) => isTauri ? command<AppSettings>("update_app_settings", { patch }) : Promise.resolve(previewSettings = { ...previewSettings, ...patch }),
  reset: () => isTauri ? command<AppSettings>("reset_app_settings") : Promise.resolve(previewSettings = { ...defaultAppSettings }),
  integration: () => isTauri ? command<IntegrationStatus>("get_integration_status") : Promise.resolve({
    platform: "web-preview", fileAssociationsDeclared: false, modernContextMenuAvailable: false,
    modernContextMenuRegistered: false, updaterConfigured: false, distribution: "web-preview", appVersion: desktopPackage.version
  }),
  openDefaultApps: () => command<void>("open_default_apps_settings"),
  downloadUpdate: (tag: string, handler: (progress: UpdateDownloadProgress) => void) => {
    const onProgress = new Channel<UpdateDownloadProgress>();
    onProgress.onmessage = handler;
    return command<DownloadedUpdate>("download_update", { tag, onProgress });
  },
  cancelDownload: () => command<void>("cancel_update_download"),
  installUpdate: (token: string) => command<void>("install_update", { token }),
  openUpdateLink: (url: string) => isTauri ? command<void>("open_update_link", { url }) : Promise.resolve(window.open(url, "_blank", "noopener,noreferrer")).then(() => undefined),
  checkForUpdates: () => isTauri ? command<UpdateCheckResult>("check_for_updates") : Promise.resolve({
    configured: false,
    status: "unavailable" as const,
    currentVersion: desktopPackage.version,
    latestVersion: desktopPackage.version,
    releaseUrl: "https://github.com/isunky/QZip/releases/latest"
  })
};
