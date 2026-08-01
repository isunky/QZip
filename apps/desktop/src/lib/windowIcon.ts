import type { AccentTheme, ThemeMode } from "@qzip/ui";
import darkAmber from "../assets/app-icons/dark-amber.png";
import darkCoral from "../assets/app-icons/dark-coral.png";
import darkCyanSlate from "../assets/app-icons/dark-cyan-slate.png";
import darkLavender from "../assets/app-icons/dark-lavender.png";
import darkMint from "../assets/app-icons/dark-mint.png";
import darkOcean from "../assets/app-icons/dark-ocean.png";
import lightAmber from "../assets/app-icons/light-amber.png";
import lightCoral from "../assets/app-icons/light-coral.png";
import lightCyanSlate from "../assets/app-icons/light-cyan-slate.png";
import lightLavender from "../assets/app-icons/light-lavender.png";
import lightMint from "../assets/app-icons/light-mint.png";
import lightOcean from "../assets/app-icons/light-ocean.png";

type ResolvedThemeMode = Exclude<ThemeMode, "system">;

const iconUrls: Record<ResolvedThemeMode, Record<AccentTheme, string>> = {
  light: { mint: lightMint, ocean: lightOcean, lavender: lightLavender, amber: lightAmber, coral: lightCoral, "cyan-slate": lightCyanSlate },
  dark: { mint: darkMint, ocean: darkOcean, lavender: darkLavender, amber: darkAmber, coral: darkCoral, "cyan-slate": darkCyanSlate }
};

export function windowIconUrl(mode: ResolvedThemeMode, accent: AccentTheme): string {
  return iconUrls[mode][accent];
}

export async function syncWindowIcon(mode: ResolvedThemeMode, accent: AccentTheme): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) return;
  const response = await fetch(windowIconUrl(mode, accent));
  if (!response.ok) throw new Error(`Unable to load ${mode}/${accent} QZip icon.`);
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().setIcon(new Uint8Array(await response.arrayBuffer()));
}
