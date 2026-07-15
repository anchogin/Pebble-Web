import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("message count query", () => {
  it("uses the folder message count endpoint with the same folderIds filter", async () => {
    const apiSource = await readFile(new URL("../../lib/api.ts", import.meta.url), "utf8");
    const querySource = await readFile(new URL("./useMessagesQuery.ts", import.meta.url), "utf8");

    expect(apiSource).toContain("getMessageCount");
    expect(apiSource).toContain("/messages/count");
    expect(apiSource).toContain("folderIds: folderIds?.join(\",\")");
    expect(querySource).toContain("messageCountQueryKey");
    expect(querySource).toContain("useMessageCountQuery");
    expect(querySource).toContain("getMessageCount(folderId!, folderIds)");
  });
});
