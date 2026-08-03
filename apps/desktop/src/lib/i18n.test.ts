import { describe, expect, it } from "vitest";
import { localize, resolveAppLocale } from "./i18n";

describe("i18n", () => {
  it("resolves explicit and system language preferences", () => {
    expect(resolveAppLocale("zh-CN", ["en-US"])).toBe("zh-CN");
    expect(resolveAppLocale("en-US", ["zh-CN"])).toBe("en-US");
    expect(resolveAppLocale("system", ["zh-Hans-CN", "en-US"])).toBe("zh-CN");
    expect(resolveAppLocale("system", ["fr-FR"])).toBe("en-US");
  });

  it("uses 轻压 for Chinese and QZip for English", () => {
    expect(localize("zh-CN", "轻压", "QZip")).toBe("轻压");
    expect(localize("en-US", "轻压", "QZip")).toBe("QZip");
  });
});
