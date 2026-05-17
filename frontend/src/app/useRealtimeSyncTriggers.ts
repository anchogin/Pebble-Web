import { useEffect, useMemo, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAccountsQuery } from "@/hooks/queries";
import { startSync, triggerSync } from "@/lib/api";
import { wsClient } from "@/lib/websocket";
import { useMailStore } from "@/stores/mail.store";
import { useUIStore } from "@/stores/ui.store";

interface SyncAccount {
  id: string;
}

const EMPTY_ACCOUNTS: SyncAccount[] = [];

export function useRealtimeSyncTriggers() {
  const queryClient = useQueryClient();
  const activeAccountId = useMailStore((s) => s.activeAccountId);
  const networkStatus = useUIStore((s) => s.networkStatus);
  const pollInterval = useUIStore((s) => s.pollInterval);
  const realtimeMode = useUIStore((s) => s.realtimeMode);
  const { data: accounts = EMPTY_ACCOUNTS } = useAccountsQuery();
  const previousNetworkStatus = useRef(networkStatus);
  const accountIds = useMemo(() => {
    const ids = accounts.map((account) => account.id).filter(Boolean);
    if (activeAccountId && !ids.includes(activeAccountId)) {
      ids.push(activeAccountId);
    }
    return ids;
  }, [accounts, activeAccountId]);

  useEffect(() => {
    if (accountIds.length === 0) return;

    const triggerAccount = (accountId: string, reason: string, ensureRunning: boolean) => {
      const trigger = () => {
        triggerSync(accountId, reason).catch(() => {});
      };

      if (ensureRunning && realtimeMode !== "manual") {
        startSync(accountId, pollInterval)
          .catch(() => {})
          .finally(trigger);
        return;
      }

      trigger();
    };

    const onFocus = () => {
      for (const accountId of accountIds) {
        triggerAccount(accountId, "window_focus", true);
      }
    };
    const onBlur = () => {
      for (const accountId of accountIds) {
        triggerAccount(accountId, "window_blur", false);
      }
    };

    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, [accountIds, pollInterval, realtimeMode]);

  useEffect(() => {
    const previous = previousNetworkStatus.current;
    previousNetworkStatus.current = networkStatus;

    if (accountIds.length === 0 || previous !== "offline" || networkStatus !== "online") return;
    for (const accountId of accountIds) {
      if (realtimeMode === "manual") {
        triggerSync(accountId, "network_online").catch(() => {});
      } else {
        startSync(accountId, pollInterval)
          .catch(() => {})
          .finally(() => {
            triggerSync(accountId, "network_online").catch(() => {});
          });
      }
    }
  }, [accountIds, networkStatus, pollInterval, realtimeMode]);

  // WebSocket connection for realtime sync notifications
  useEffect(() => {
    wsClient.connect();

    const handler = (msg: any) => {
      if (msg.type === "sync_complete" || msg.type === "new_mail") {
        queryClient.invalidateQueries({ queryKey: ["messages"] });
        queryClient.invalidateQueries({ queryKey: ["folders"] });
        queryClient.invalidateQueries({ queryKey: ["threads"] });
      }
    };

    wsClient.on("*", handler);

    return () => {
      wsClient.off("*", handler);
      wsClient.disconnect();
    };
  }, [queryClient]);
}
