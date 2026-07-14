export function consumeMessageIdParam(rawUrl: string): { messageId: string; nextUrl: string } | null {
  const url = new URL(rawUrl, "http://pebble.local");
  const messageId = url.searchParams.get("messageId")?.trim();
  if (!messageId) {
    return null;
  }

  url.searchParams.delete("messageId");
  return {
    messageId,
    nextUrl: `${url.pathname}${url.search}${url.hash}`,
  };
}
