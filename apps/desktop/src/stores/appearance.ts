import { create } from "zustand";
import type { AccentTheme, ThemeMode } from "@qzip/ui";

interface AppearanceState {
  mode: ThemeMode;
  accent: AccentTheme;
  setMode: (mode: ThemeMode) => void;
  setAccent: (accent: AccentTheme) => void;
}

export function resolveThemeMode(
  mode: ThemeMode,
  systemDark: boolean
): Exclude<ThemeMode, "system"> {
  return mode === "system" ? (systemDark ? "dark" : "light") : mode;
}

export const useAppearanceStore = create<AppearanceState>((set) => ({
  mode: "light",
  accent: "mint",
  setMode: (mode) => set({ mode }),
  setAccent: (accent) => set({ accent })
}));
