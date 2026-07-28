import { describe, expect, it } from "vitest";
import { suggestCreateOutputLocally } from "./ArchivePages";

describe("suggestCreateOutputLocally", () => {
  it("builds an archive next to a selected file", () => {
    expect(suggestCreateOutputLocally(
      ["D:\\资料\\README.md"],
      "sevenZip"
    )).toBe("D:\\资料\\README.7z");
  });

  it("uses the multi-selection name and compound archive extension", () => {
    expect(suggestCreateOutputLocally(
      ["D:\\资料\\一.txt", "D:\\资料\\二.txt"],
      "tarGz"
    )).toBe("D:\\资料\\压缩文件.tar.gz");
  });

  it("handles a selected directory with a trailing separator", () => {
    expect(suggestCreateOutputLocally(
      ["D:\\资料\\项目文件\\"],
      "zip"
    )).toBe("D:\\资料\\项目文件.zip");
  });
});
