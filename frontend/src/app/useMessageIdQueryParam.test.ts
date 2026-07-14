import { afterEach, describe, expect, it, vi } from "vitest";
import { consumeMessageIdParam } from "./messageDeepLink";

describe("openMessageIdFromCurrentUrl", () => {
  afterEach(() => vi.restoreAllMocks());

  it("opens the message from the messageId query parameter and removes only that parameter", () => {
    const openMessage = vi.fn();
    const consumed = consumeMessageIdParam("https://mail.example.com/?messageId=msg-123&theme=dark");

    if (consumed) {
      openMessage(consumed.messageId);
    }

    expect(openMessage).toHaveBeenCalledWith("msg-123");
    expect(consumed?.nextUrl).toBe("/?theme=dark");
  });

  it("does nothing when messageId is missing or blank", () => {
    const openMessage = vi.fn();

    const consumed = consumeMessageIdParam("https://mail.example.com/?messageId=   ");

    expect(consumed).toBeNull();
    expect(openMessage).not.toHaveBeenCalled();
  });

  it("routes messageId links to the standalone message view", async () => {
    const [hookSource, layoutSource, uiStoreSource, sidebarSource] = await Promise.all([
      import("node:fs/promises").then(({ readFile }) =>
        readFile(new URL("./useMessageIdQueryParam.ts", import.meta.url), "utf8"),
      ),
      import("node:fs/promises").then(({ readFile }) =>
        readFile(new URL("./Layout.tsx", import.meta.url), "utf8"),
      ),
      import("node:fs/promises").then(({ readFile }) =>
        readFile(new URL("../stores/ui.store.ts", import.meta.url), "utf8"),
      ),
      import("node:fs/promises").then(({ readFile }) =>
        readFile(new URL("../components/Sidebar.tsx", import.meta.url), "utf8"),
      ),
    ]);

    expect(hookSource).toContain("openMessageStandalone");
    expect(layoutSource).toContain('displayedView === "message"');
    expect(uiStoreSource).toContain('"message"');
    expect(sidebarSource).toContain('activeView === "message"');
  });
});
