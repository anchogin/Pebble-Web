import { useMutation } from "@tanstack/react-query";
import { triggerSync } from "@/lib/api";

export function useSyncMutation() {
  return useMutation({
    mutationFn: (accountId: string) => triggerSync(accountId, "manual"),
    // Data refresh is driven by mail:sync-complete and mail:new events in StatusBar
  });
}
