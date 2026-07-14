import { useEffect } from "react";
import { useUIStore } from "../stores/ui.store";
import { consumeMessageIdParam } from "./messageDeepLink";

type OpenMessage = (messageId: string) => void;

export function openMessageIdFromCurrentUrl(openMessage: OpenMessage): boolean {
  const consumed = consumeMessageIdParam(window.location.href);
  if (!consumed) {
    return false;
  }

  openMessage(consumed.messageId);
  window.history.replaceState(null, "", consumed.nextUrl);
  return true;
}

export function useMessageIdQueryParam() {
  useEffect(() => {
    openMessageIdFromCurrentUrl(useUIStore.getState().openMessageStandalone);
  }, []);
}
