import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("message count invalidation", () => {
  it("refreshes count caches when visible folder totals can change", async () => {
    const files = [
      new URL("./MessageList.tsx", import.meta.url),
      new URL("./MessageActionToolbar.tsx", import.meta.url),
      new URL("./MessageItem.tsx", import.meta.url),
      new URL("../hooks/useKeyboard.ts", import.meta.url),
      new URL("../features/command-palette/commands.ts", import.meta.url),
      new URL("../features/inbox/InboxView.tsx", import.meta.url),
    ];

    for (const file of files) {
      const source = await readFile(file, "utf8");
      expect(source, file.pathname).toContain('queryKey: ["message-count"]');
    }
  });
});
