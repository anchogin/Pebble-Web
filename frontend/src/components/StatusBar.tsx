import { useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { AlertCircle, AppWindow, Clock, RefreshCw } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useUIStore, type RealtimeStatus } from "../stores/ui.store";
import { useMailStore } from "@/stores/mail.store";
import { stopSync } from "@/lib/api";
import { useSyncMutation } from "@/hooks/mutations/useSyncMutation";
import {
  useAccountsQuery,
  useFoldersForAccountsQuery,
  useMessageCountQuery,
  usePendingMailOpsSummary,
} from "@/hooks/queries";
import { folderIdsForSelection } from "@/lib/folderAggregation";

interface SyncAccount {
  id: string;
}

const EMPTY_ACCOUNTS: SyncAccount[] = [];

export default function StatusBar() {
  const { t } = useTranslation();
  const syncStatus = useUIStore((s) => s.syncStatus);
  const setSyncStatus = useUIStore((s) => s.setSyncStatus);
  const networkStatus = useUIStore((s) => s.networkStatus);
  const lastMailError = useUIStore((s) => s.lastMailError);
  const realtimeStatusByAccount = useUIStore((s) => s.realtimeStatusByAccount);
  const notificationsEnabled = useUIStore((s) => s.notificationsEnabled);
  const keepRunningInBackground = useUIStore((s) => s.keepRunningInBackground);
  const setKeepRunningInBackground = useUIStore((s) => s.setKeepRunningInBackground);
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const activeView = useUIStore((s) => s.activeView);
  const searchResultCount = useUIStore((s) => s.searchResultCount);
  const activeAccountId = useMailStore((s) => s.activeAccountId);
  const activeFolderId = useMailStore((s) => s.activeFolderId);
  const syncMutation = useSyncMutation();
  const queryClient = useQueryClient();
  const { data: accounts = EMPTY_ACCOUNTS } = useAccountsQuery();
  const { data: pendingOpsSummary } = usePendingMailOpsSummary(activeAccountId);
  const syncStatusRef = useRef(syncStatus);
  const syncAccountIds = activeAccountId ? [activeAccountId] : accounts.map((account) => account.id);
  const folderAccountIds = useMemo(
    () => activeAccountId ? [activeAccountId] : accounts.map((account) => account.id),
    [accounts, activeAccountId],
  );
  const { data: folders = [] } = useFoldersForAccountsQuery(folderAccountIds);
  const selectedFolderIds = folderIdsForSelection(activeFolderId, folders);
  const queryFolderId = selectedFolderIds[0] ?? null;
  const queryFolderIds = selectedFolderIds.length > 1 ? selectedFolderIds : undefined;
  const { data: messageCount } = useMessageCountQuery(queryFolderId, queryFolderIds);
  const displayedMessageTotal = activeView === "search" ? searchResultCount : messageCount?.total;

  useEffect(() => {
    syncStatusRef.current = syncStatus;
  }, [syncStatus]);

  function updateSyncStatus(status: typeof syncStatus) {
    syncStatusRef.current = status;
    setSyncStatus(status);
  }

  // In web mode, we poll for updates instead of listening to Tauri events.
  // For now, rely on react-query's refetch intervals.

  async function handleSync() {
    if (syncAccountIds.length === 0) return;
    if (syncStatus === "syncing") {
      try { await Promise.all(syncAccountIds.map((accountId) => stopSync(accountId))); } catch { /* ignored */ }
      updateSyncStatus("idle");
    } else {
      updateSyncStatus("syncing");
      try {
        await Promise.all(syncAccountIds.map((accountId) => syncMutation.mutateAsync(accountId)));
        updateSyncStatus("idle");
        queryClient.invalidateQueries({ queryKey: ["folders"] });
        queryClient.invalidateQueries({ queryKey: ["messages"] });
        queryClient.invalidateQueries({ queryKey: ["message-count"] });
        queryClient.invalidateQueries({ queryKey: ["threads"] });
        queryClient.invalidateQueries({ queryKey: ["folder-unread-counts"] });
      } catch {
        updateSyncStatus("error");
      }
    }
  }

  const syncText = {
    idle: t("status.ready", "Ready"),
    syncing: t("status.syncing", "Syncing..."),
    error: t("status.syncError", "Sync error"),
  }[syncStatus];
  const realtimeStatus = activeAccountId ? realtimeStatusByAccount[activeAccountId] : undefined;
  const realtimeStatusText = getRealtimeStatusText(realtimeStatus, t);

  const pendingRemoteWrites = pendingOpsSummary?.total_active_count ?? 0;
  const failedRemoteWrites = pendingOpsSummary?.failed_count ?? 0;
  const retryingRemoteWrites = pendingOpsSummary?.in_progress_count ?? 0;
  const backgroundLabel = keepRunningInBackground
    ? t("status.exitOnClose", "Exit on close")
    : t("status.keepRunningInBackground", "Keep running in background on close");
  const pendingRemoteText = retryingRemoteWrites > 0
    ? t("status.remoteWritesRetrying", "{{count}} remote writes retrying", { count: retryingRemoteWrites })
    : failedRemoteWrites > 0
      ? t("status.remoteWritesPending", "{{count}} remote writes pending", { count: pendingRemoteWrites })
      : t("status.remoteWritesQueued", "{{count}} remote writes queued", { count: pendingRemoteWrites });

  return (
    <footer
      className="flex items-center px-3 h-6 text-xs border-t gap-3"
      style={{
        backgroundColor: "var(--color-statusbar-bg)",
        borderColor: "var(--color-border)",
        color: "var(--color-text-secondary)",
        position: "relative",
      }}
    >
      {displayedMessageTotal !== undefined && (
        <span
          aria-label={t("status.messageTotal", "{{count}} messages", { count: displayedMessageTotal })}
          title={t("status.messageTotal", "{{count}} messages", { count: displayedMessageTotal })}
          className="truncate"
          style={{
            position: "absolute",
            left: sidebarCollapsed ? "48px" : "200px",
            maxWidth: "160px",
            paddingRight: "12px",
            color: "var(--color-text-secondary)",
            pointerEvents: "none",
          }}
        >
          {t("status.messageTotal", "{{count}} messages", { count: displayedMessageTotal })}
        </span>
      )}
      {networkStatus === "offline" ? (
        <span
          role="status"
          aria-live="polite"
          aria-atomic="true"
          className="flex items-center gap-1"
          style={{ color: "var(--color-error, #ef4444)" }}
        >
          <svg aria-hidden="true" focusable="false" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="1" y1="1" x2="23" y2="23" />
            <path d="M16.72 11.06A10.94 10.94 0 0 1 19 12.55" />
            <path d="M5 12.55a10.94 10.94 0 0 1 5.17-2.39" />
            <path d="M10.71 5.05A16 16 0 0 1 22.56 9" />
            <path d="M1.42 9a15.91 15.91 0 0 1 4.7-2.88" />
            <path d="M8.53 16.11a6 6 0 0 1 6.95 0" />
            <line x1="12" y1="20" x2="12.01" y2="20" />
          </svg>
          {t("status.offline", "Offline")}
        </span>
      ) : (
        <>
          <span role="status" aria-live="polite" aria-atomic="true">{syncText}</span>
          {realtimeStatusText && (
            <span
              role="status"
              aria-live="polite"
              aria-atomic="true"
              aria-label={realtimeStatusText}
              className="truncate"
              title={realtimeStatus?.message ?? realtimeStatusText}
              style={{ maxWidth: "180px" }}
            >
              {realtimeStatusText}
            </span>
          )}
          <button
            onClick={handleSync}
            disabled={syncAccountIds.length === 0}
            title={syncStatus === "syncing" ? t("status.stopSync") : t("status.syncNow")}
            aria-label={syncStatus === "syncing" ? t("status.stopSync") : t("status.syncNow")}
            style={{
              background: "none",
              border: "none",
              cursor: syncAccountIds.length > 0 ? "pointer" : "default",
              padding: "2px",
              color: "var(--color-text-secondary)",
              display: "flex",
              alignItems: "center",
              opacity: syncAccountIds.length > 0 ? 1 : 0.4,
            }}
          >
            <RefreshCw
              aria-hidden="true"
              size={13}
              style={{
                animation: syncStatus === "syncing" ? "spin 1s linear infinite" : "none",
              }}
            />
          </button>
          {pendingRemoteWrites > 0 && (
            <span
              role={failedRemoteWrites > 0 ? "alert" : "status"}
              aria-live={failedRemoteWrites > 0 ? "assertive" : "polite"}
              aria-atomic="true"
              className="flex items-center gap-1 truncate"
              title={pendingOpsSummary?.last_error ?? pendingRemoteText}
              style={{
                color: failedRemoteWrites > 0
                  ? "var(--color-warning, #d97706)"
                  : "var(--color-text-secondary)",
                maxWidth: "220px",
              }}
            >
              {failedRemoteWrites > 0 ? <AlertCircle aria-hidden="true" size={13} /> : <Clock aria-hidden="true" size={13} />}
              <span className="truncate">{pendingRemoteText}</span>
            </span>
          )}
        </>
      )}

      {lastMailError && (
        <span
          role="alert"
          aria-live="assertive"
          aria-atomic="true"
          className="truncate"
          style={{ color: "var(--color-error, #ef4444)" }}
        >
          {lastMailError}
        </span>
      )}

      <span className="ml-auto flex items-center gap-1">
        <button
          type="button"
          onClick={() => setKeepRunningInBackground(!keepRunningInBackground)}
          aria-label={backgroundLabel}
          aria-pressed={keepRunningInBackground}
          title={backgroundLabel}
          className="inline-flex items-center justify-center"
          style={{
            width: "20px",
            height: "20px",
            border: "1px solid transparent",
            borderRadius: "4px",
            background: keepRunningInBackground
              ? "color-mix(in srgb, var(--color-accent) 16%, transparent)"
              : "transparent",
            color: keepRunningInBackground
              ? "var(--color-accent)"
              : "var(--color-text-secondary)",
            cursor: "pointer",
            padding: 0,
          }}
        >
          <AppWindow aria-hidden="true" size={13} />
        </button>
        {notificationsEnabled && (
          <svg aria-hidden="true" focusable="false" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
            <path d="M13.73 21a2 2 0 0 1-3.46 0" />
          </svg>
        )}
      </span>
    </footer>
  );
}

function getRealtimeStatusText(
  status: RealtimeStatus | undefined,
  t: TFunction,
) {
  if (!status) return null;

  if (status.message) {
    return status.message;
  }

  switch (status.mode) {
    case "realtime":
      return t("status.realtimeConnected", "Realtime connected");
    case "polling":
      return t("status.realtimePolling", "Polling");
    case "manual":
      return t("status.realtimeManual", "Manual only");
    case "backoff":
      return t("status.realtimeBackoff", "Retrying");
    case "auth_required":
      return t("status.realtimeAuthRequired", "Reconnect required");
    case "offline":
      return t("status.offline", "Offline");
    case "error":
      return t("status.realtimeError", "Realtime error");
  }
}
