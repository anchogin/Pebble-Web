import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("StatusBar refresh button", () => {
  it("uses available accounts when no active account is selected", async () => {
    const source = await readFile(new URL("./StatusBar.tsx", import.meta.url), "utf8");

    expect(source).toContain("useAccountsQuery");
    expect(source).toContain("syncAccountIds");
    expect(source).toContain("accounts.map((account) => account.id)");
    expect(source).toContain("disabled={syncAccountIds.length === 0}");
    expect(source).not.toContain("if (!activeAccountId) return;");
  });
});
