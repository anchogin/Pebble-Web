import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("Pebble public URL settings placement", () => {
  it("keeps publicUrl owned by General settings instead of MagicPush", async () => {
    const [generalTab, publicUrlSettings, magicPushTab, api] = await Promise.all([
      readFile(new URL("./GeneralTab.tsx", import.meta.url), "utf8"),
      readFile(new URL("./PublicUrlSettings.tsx", import.meta.url), "utf8"),
      readFile(new URL("./MagicPushTab.tsx", import.meta.url), "utf8"),
      readFile(new URL("../../lib/api.ts", import.meta.url), "utf8"),
    ]);

    expect(generalTab).toContain("PublicUrlSettings");
    expect(publicUrlSettings).toContain("settings.publicUrl");
    expect(publicUrlSettings).toContain("getGeneralSettings");
    expect(publicUrlSettings).toContain("saveGeneralSettings");
    expect(magicPushTab).not.toContain("magicpush-public-url");
    expect(magicPushTab).not.toContain("publicUrl");
    expect(api).toContain("getGeneralSettings");
    expect(api).toContain("saveGeneralSettings");
    expect(api).not.toContain("publicUrl: string;\n  hasToken");
  });
});
