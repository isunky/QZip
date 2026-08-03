import { createContext, useContext } from "react";
import type { LanguagePreference } from "../contracts/settings";

export type AppLocale = "zh-CN" | "en-US";

export interface I18nValue {
  locale: AppLocale;
  brandName: string;
  text: (zhCN: string, enUS: string) => string;
}

export const I18nContext = createContext<I18nValue>({
  locale: "zh-CN",
  brandName: "轻压",
  text: (zhCN) => zhCN
});

export function resolveAppLocale(
  preference: LanguagePreference,
  languages: readonly string[] = typeof navigator === "undefined" ? [] : navigator.languages
): AppLocale {
  if (preference === "zh-CN" || preference === "en-US") return preference;
  return languages.some((language) => language.toLowerCase().startsWith("zh")) ? "zh-CN" : "en-US";
}

export function localize(locale: AppLocale, zhCN: string, enUS: string) {
  return locale === "zh-CN" ? zhCN : enUS;
}

export function useI18n() {
  return useContext(I18nContext);
}
