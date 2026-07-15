import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("search status count", () => {
  it("publishes search result counts for the status bar", async () => {
    const source = await readFile(new URL("./SearchView.tsx", import.meta.url), "utf8");

    expect(source).toContain("setSearchResultCount");
    expect(source).toContain("setSearchResultCount(results.length)");
    expect(source).toContain("setSearchResultCount(0)");
  });

  it("status bar uses search result count while search view is active", async () => {
    const statusBar = await readFile(new URL("../../components/StatusBar.tsx", import.meta.url), "utf8");
    const uiStore = await readFile(new URL("../../stores/ui.store.ts", import.meta.url), "utf8");

    expect(uiStore).toContain("searchResultCount: number");
    expect(uiStore).toContain("setSearchResultCount");
    expect(statusBar).toContain("activeView === \"search\"");
    expect(statusBar).toContain("searchResultCount");
    expect(statusBar).toContain("activeView === \"search\" ? searchResultCount : messageCount?.total");
  });
});
