import type { ArchiveFormat, CompressionProfile, ConflictPolicy } from "./archive";

export type ThemeMode = "light" | "dark" | "system";
export type AccentTheme = "mint" | "ocean" | "lavender" | "amber" | "coral" | "cyan-slate";
export type UiScale = "scale90" | "scale100" | "scale110" | "scale125";
export type ListDensity = "comfortable" | "compact";
export type LanguagePreference = "system" | "zh-CN" | "en-US";

export interface AppSettings {
  schemaVersion: number;
  language: LanguagePreference;
  themeMode: ThemeMode;
  accentTheme: AccentTheme;
  uiScale: UiScale;
  listDensity: ListDensity;
  reduceMotion: boolean;
  defaultFormat: ArchiveFormat;
  compressionProfile: CompressionProfile;
  conflictPolicy: ConflictPolicy;
  extractToNamedFolder: boolean;
  avoidDuplicateRootFolder: boolean;
  openFolderAfterExtract: boolean;
  testAfterCreate: boolean;
  taskNotificationsEnabled: boolean;
  notifyOnSuccess: boolean;
  notifyOnFailure: boolean;
  checkUpdatesOnStartup: boolean;
  telemetryEnabled: false;
}

export type AppSettingsPatch = Partial<Omit<AppSettings, "schemaVersion" | "telemetryEnabled">>;

export interface IntegrationStatus {
  platform: string;
  fileAssociationsDeclared: boolean;
  modernContextMenuAvailable: boolean;
  modernContextMenuRegistered: boolean;
  updaterConfigured: boolean;
  distribution: string;
  appVersion: string;
}

export interface UpdateCheckResult { configured: boolean; status: "ready" | "unconfigured"; }

export const defaultAppSettings: AppSettings = {
  schemaVersion: 1,
  language: "zh-CN",
  themeMode: "light",
  accentTheme: "mint",
  uiScale: "scale100",
  listDensity: "comfortable",
  reduceMotion: false,
  defaultFormat: "sevenZip",
  compressionProfile: "balanced",
  conflictPolicy: "rename",
  extractToNamedFolder: true,
  avoidDuplicateRootFolder: true,
  openFolderAfterExtract: false,
  testAfterCreate: true,
  taskNotificationsEnabled: false,
  notifyOnSuccess: true,
  notifyOnFailure: true,
  checkUpdatesOnStartup: false,
  telemetryEnabled: false
};

export const uiScaleFactor: Record<UiScale, number> = {
  scale90: 0.9, scale100: 1, scale110: 1.1, scale125: 1.25
};
