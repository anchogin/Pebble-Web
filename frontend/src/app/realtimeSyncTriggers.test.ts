import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("realtime sync cache refresh", () => {
  it("does not trigger regular mail pulls from the frontend", async () => {
    const source = await readFile(new URL("./useRealtimeSyncTriggers.ts", import.meta.url), "utf8");

    expect(source).not.toContain("startSync(");
    expect(source).not.toContain("triggerSync(");
  });

  it("does not trigger regular mail pulls after account setup", async () => {
    const source = await readFile(new URL("../components/AccountSetup.tsx", import.meta.url), "utf8");

    expect(source).not.toContain("startSync(");
    expect(source).not.toContain("syncPollInterval");
  });

  it("refreshes mail caches for new mail and completed sync progress", async () => {
    const source = await readFile(new URL("./useRealtimeSyncTriggers.ts", import.meta.url), "utf8");

    expect(source).toContain('msg.type === "new_mail"');
    expect(source).toContain('msg.type === "sync_progress"');
    expect(source).toContain('msg.status === "completed"');
    expect(source).toContain('["messages"]');
    expect(source).toContain('["message-count"]');
    expect(source).toContain('["folders"]');
    expect(source).toContain('["threads"]');
    expect(source).toContain('["folder-unread-counts"]');
  });
});
