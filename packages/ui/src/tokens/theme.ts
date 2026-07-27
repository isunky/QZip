export const accentThemes = [
  "mint",
  "ocean",
  "lavender",
  "amber",
  "coral",
  "cyan-slate"
] as const;

export type AccentTheme = (typeof accentThemes)[number];
export type ThemeMode = "light" | "dark" | "system";
