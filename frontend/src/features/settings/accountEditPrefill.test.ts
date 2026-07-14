import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vitest";

describe("account edit prefill", () => {
  it("reads camelCase account config fields returned by the web backend", async () => {
    const [accountsTab, ipcTypes] = await Promise.all([
      readFile(new URL("./AccountsTab.tsx", import.meta.url), "utf8"),
      readFile(new URL("../../lib/ipc-types.ts", import.meta.url), "utf8"),
    ]);

    expect(ipcTypes).toContain("imapHost?: string");
    expect(ipcTypes).toContain("smtpHost?: string");
    expect(ipcTypes).toContain("username?: string");
    expect(accountsTab).toContain("config.imapHost");
    expect(accountsTab).toContain("config.smtpHost");
    expect(accountsTab).toContain("config.username");
    expect(accountsTab).not.toContain("config.imap_host");
    expect(accountsTab).not.toContain("config.smtp_host");
  });
});
