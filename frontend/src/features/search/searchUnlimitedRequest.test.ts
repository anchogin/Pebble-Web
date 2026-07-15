import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("search request limits", () => {
  it("does not send a default limit from the frontend search view", async () => {
    const apiSource = await readFile(new URL("../../lib/api.ts", import.meta.url), "utf8");
    const searchViewSource = await readFile(new URL("./SearchView.tsx", import.meta.url), "utf8");

    expect(apiSource).toContain("searchMessages(");
    expect(apiSource).toContain("advancedSearch(");
    expect(searchViewSource).toContain("searchMessages(trimmed)");
    expect(searchViewSource).not.toContain("searchMessages(trimmed, 50)");
    expect(searchViewSource).not.toContain("advancedSearch({ ...filters, text: trimmed || undefined }, 50)");
  });
});
