import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { wsClient } from "@/lib/websocket";

export function useRealtimeSyncTriggers() {
  const queryClient = useQueryClient();

  // WebSocket connection for realtime sync notifications
  useEffect(() => {
    wsClient.connect();

    const handler = (msg: any) => {
      const shouldRefresh = msg.type === "new_mail"
        || msg.type === "sync_complete"
        || (msg.type === "sync_progress" && msg.status === "completed");

      if (shouldRefresh) {
        queryClient.invalidateQueries({ queryKey: ["messages"] });
        queryClient.invalidateQueries({ queryKey: ["message-count"] });
        queryClient.invalidateQueries({ queryKey: ["folders"] });
        queryClient.invalidateQueries({ queryKey: ["threads"] });
        queryClient.invalidateQueries({ queryKey: ["folder-unread-counts"] });
      }
    };

    wsClient.on("*", handler);

    return () => {
      wsClient.off("*", handler);
      wsClient.disconnect();
    };
  }, [queryClient]);
}
