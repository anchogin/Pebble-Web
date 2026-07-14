import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";
import { serializeRuleActions } from "./rule-json";

describe("rule MoveToFolder targets", () => {
  it("keeps the move target as a folder name", () => {
    const actions = serializeRuleActions([
      { type: "MoveToFolder", value: "Projects" },
    ]);

    expect(actions).toBe('[{"type":"MoveToFolder","value":"Projects"}]');
    expect(actions).not.toContain("folderId");
  });

  it("lets global rules choose folders from all accounts", async () => {
    const source = await readFile(new URL("./RulesTab.tsx", import.meta.url), "utf8");

    expect(source).toContain("useFoldersForAccountsQuery");
    expect(source).toContain("accounts.map((account) => account.id)");
    expect(source).not.toContain('action.type === "MoveToFolder" && !form.account_id');
    expect(source).not.toContain('t("rules.selectAccountFirst"');
  });
});
