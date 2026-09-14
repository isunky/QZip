import { describe, expect, it } from "vitest";
import { joinOutputPath, splitOutputPath, suggestCreateOutputLocally } from "./archivePath";

describe("cross-platform output paths", () => {
  it("retains the POSIX root directory", () => {
    expect(splitOutputPath("/归档.zip")).toEqual({ directory: "/", name: "归档.zip" });
    expect(joinOutputPath("/", "归档.7z")).toBe("/归档.7z");
    expect(suggestCreateOutputLocally(["/文件.txt"], "zip")).toBe("/文件.zip");
  });
  it("preserves macOS spaces and Unicode names", () => {
    expect(suggestCreateOutputLocally(["/Users/test/中文 📦/文件.txt"], "tarGz")).toBe("/Users/test/中文 📦/文件.tar.gz");
  });
  it("retains Windows path separators", () => {
    expect(suggestCreateOutputLocally(["C:\\资料\\文件.txt"], "zip")).toBe("C:\\资料\\文件.zip");
  });
});
