import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("StatusBar message count", () => {
  it("derives the current folder filter and aligns the count after the sidebar", async () => {
    const source = await readFile(new URL("./StatusBar.tsx", import.meta.url), "utf8");

    expect(source).toContain("activeFolderId");
    expect(source).toContain("useFoldersForAccountsQuery");
    expect(source).toContain("folderIdsForSelection(activeFolderId, folders)");
    expect(source).toContain("useMessageCountQuery(queryFolderId, queryFolderIds)");
    expect(source).toContain("sidebarCollapsed");
    expect(source).toContain("left: sidebarCollapsed ? \"48px\" : \"200px\"");
    expect(source).not.toContain("paddingLeft: \"12px\"");
    expect(source).toContain("messageCount?.total");
  });

  it("defines localized message total labels", async () => {
    const en = JSON.parse(await readFile(new URL("../locales/en.json", import.meta.url), "utf8"));
    const zh = JSON.parse(await readFile(new URL("../locales/zh.json", import.meta.url), "utf8"));

    expect(en.status.messageTotal).toBe("{{count}} messages");
    expect(zh.status.messageTotal).toBe("{{count}} 封邮件");
  });
});
