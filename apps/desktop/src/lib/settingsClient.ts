import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, AppSettingsPatch, IntegrationStatus, UpdateCheckResult } from "../contracts/settings";
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
  checkForUpdates: () => isTauri ? command<UpdateCheckResult>("check_for_updates") : Promise.resolve({ configured: false, status: "unconfigured" as const })
};
