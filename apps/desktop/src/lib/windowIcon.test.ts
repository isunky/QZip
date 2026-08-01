import { describe, expect, it } from "vitest";
import { syncWindowIcon, windowIconUrl } from "./windowIcon";

describe("QZip runtime icon variants", () => {
  it("selects a distinct asset for light and dark accent combinations", () => {
    expect(windowIconUrl("light", "mint")).toContain("light-mint");
    expect(windowIconUrl("dark", "ocean")).toContain("dark-ocean");
    expect(windowIconUrl("light", "lavender")).not.toBe(windowIconUrl("dark", "lavender"));
  });

  it("does nothing in the web preview", async () => {
    await expect(syncWindowIcon("light", "mint")).resolves.toBeUndefined();
  });
});
