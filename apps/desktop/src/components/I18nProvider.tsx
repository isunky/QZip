import type { ReactNode } from "react";
import { I18nContext, localize, type AppLocale } from "../lib/i18n";

export function I18nProvider({ locale, children }: { locale: AppLocale; children: ReactNode }) {
  return (
    <I18nContext.Provider value={{
      locale,
      brandName: localize(locale, "轻压", "QZip"),
      text: (zhCN, enUS) => localize(locale, zhCN, enUS)
    }}>
      {children}
    </I18nContext.Provider>
  );
}
