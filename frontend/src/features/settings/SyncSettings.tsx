import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useConfirmStore } from "@/stores/confirm.store";
import { updateSyncConfig, cancelSync, type SyncConfig } from "@/lib/api";
import { useToastStore } from "@/stores/toast.store";
import { inputStyle } from "@/styles/form";
import { wsClient } from "@/lib/websocket";

interface Props {
  accountId: string;
  currentStrategy?: 'recent' | 'all' | 'since_date';
  currentSinceDate?: string;
  onRefresh?: () => void;
  onSave?: () => void;
}

interface LiveProgress {
  status: string;
  phase: string;
  message?: string;
  current: number;
  total?: number;
  percentage?: number;
}

export default function SyncSettings({
  accountId,
  currentStrategy = 'recent',
  currentSinceDate,
  onRefresh,
  onSave,
}: Props) {
  const { t } = useTranslation();
  const [config, setConfig] = useState<SyncConfig>({
    syncStrategy: currentStrategy,
    syncSinceDate: currentSinceDate,
  });
  const [saving, setSaving] = useState(false);
  const [liveProgress, setLiveProgress] = useState<LiveProgress | null>(null);
  const accountIdRef = useRef(accountId);

  useEffect(() => {
    accountIdRef.current = accountId;
  }, [accountId]);

  useEffect(() => {
    setConfig({
      syncStrategy: currentStrategy,
      syncSinceDate: currentSinceDate,
    });
  }, [currentStrategy, currentSinceDate]);

  useEffect(() => {
    const handleStarted = (msg: any) => {
      if (msg.account_id !== accountIdRef.current) return;
      setLiveProgress({ status: "started", phase: msg.phase ?? "initial", message: undefined, current: 0 });
    };

    const handleProgress = (msg: any) => {
      if (msg.account_id !== accountIdRef.current) return;
      setLiveProgress({
        status: msg.status ?? "syncing",
        phase: msg.phase ?? "",
        message: msg.message,
        current: msg.progress?.current ?? 0,
        total: msg.progress?.total,
        percentage: msg.progress?.percentage,
      });
    };

    const handleComplete = (msg: any) => {
      if (msg.account_id !== accountIdRef.current) return;
      setLiveProgress((prev) =>
        prev ? { ...prev, status: "completed", message: t("settings.syncCompleted", "Sync completed") } : null
      );
      setTimeout(() => setLiveProgress(null), 3000);
    };

    const handleError = (msg: any) => {
      if (msg.account_id !== accountIdRef.current) return;
      setLiveProgress((prev) =>
        prev ? { ...prev, status: "error", message: msg.message ?? t("settings.syncFailed", "Sync failed") } : null
      );
    };

    wsClient.on("sync_started", handleStarted);
    wsClient.on("sync_progress", handleProgress);
    wsClient.on("sync_complete", handleComplete);
    wsClient.on("sync_error", handleError);

    return () => {
      wsClient.off("sync_started", handleStarted);
      wsClient.off("sync_progress", handleProgress);
      wsClient.off("sync_complete", handleComplete);
      wsClient.off("sync_error", handleError);
    };
  }, [t]);

  const handleSave = async () => {
    setSaving(true);
    try {
      const result = await updateSyncConfig(accountId, config);
      useToastStore.getState().addToast({
        message: result?.message || t("settings.syncConfigSaved", "Sync settings saved. Restarting sync..."),
        type: "success",
      });
      onSave?.();
      onRefresh?.();
    } catch (err: any) {
      useToastStore.getState().addToast({
        message: err.response?.data?.error || t("settings.syncConfigSaveFailed", "Failed to save sync settings"),
        type: "error",
      });
    } finally {
      setSaving(false);
    }
  };

  const handleCancelSync = async () => {
    const confirmed = await useConfirmStore.getState().confirm({
      title: t("settings.cancelSync", "Cancel Sync"),
      message: t("settings.cancelSyncConfirm", "Are you sure you want to cancel the current sync? Already synced messages will be kept."),
      destructive: true,
    });

    if (confirmed) {
      try {
        await cancelSync(accountId);
        useToastStore.getState().addToast({
          message: t("settings.syncCancelled", "Sync cancelled"),
          type: "info",
        });
        setLiveProgress(null);
        onRefresh?.();
      } catch (err) {
        useToastStore.getState().addToast({
          message: t("settings.cancelSyncFailed", "Failed to cancel sync"),
          type: "error",
        });
      }
    }
  };

  const isSyncing = liveProgress !== null && liveProgress.status !== "completed" && liveProgress.status !== "error";
  const isCompleted = liveProgress?.status === "completed";
  const isError = liveProgress?.status === "error";

  return (
    <div style={{ padding: "20px" }}>
      <h3 style={{ margin: "0 0 20px", fontSize: "15px", fontWeight: 600 }}>
        {t("settings.syncStrategy", "Sync Strategy")}
      </h3>

      {liveProgress && (
        <div
          style={{
            marginBottom: "20px",
            padding: "16px",
            borderRadius: "8px",
            backgroundColor: isError
              ? "rgba(239, 68, 68, 0.1)"
              : isCompleted
              ? "rgba(34, 197, 94, 0.1)"
              : "rgba(59, 130, 246, 0.1)",
            border: `1px solid ${
              isError
                ? "rgba(239, 68, 68, 0.3)"
                : isCompleted
                ? "rgba(34, 197, 94, 0.3)"
                : "rgba(59, 130, 246, 0.3)"
            }`,
          }}
        >
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
            <div style={{ flex: 1 }}>
              <p
                style={{
                  margin: 0,
                  fontSize: "14px",
                  fontWeight: 500,
                  color: isError ? "#ef4444" : isCompleted ? "#22c55e" : "var(--color-text-primary)",
                }}
              >
                {isCompleted
                  ? t("settings.syncCompleted", "Sync completed")
                  : isError
                  ? t("settings.syncFailed", "Sync failed")
                  : t("settings.syncing", "Syncing mail...")}
              </p>
              {liveProgress.message && (
                <p style={{ margin: "4px 0 0", fontSize: "13px", color: "var(--color-text-secondary)" }}>
                  {liveProgress.message}
                </p>
              )}
              <div style={{ marginTop: "10px" }}>
                <div
                  style={{
                    width: "100%",
                    height: "5px",
                    backgroundColor: "var(--color-border)",
                    borderRadius: "3px",
                    overflow: "hidden",
                  }}
                >
                  {liveProgress.total !== undefined && liveProgress.total > 0 ? (
                    <div
                      style={{
                        width: `${liveProgress.percentage ?? 0}%`,
                        height: "100%",
                        backgroundColor: isError ? "#ef4444" : isCompleted ? "#22c55e" : "var(--color-accent)",
                        borderRadius: "3px",
                        transition: "width 0.3s ease",
                      }}
                    />
                  ) : (
                    <>
                      <style>{`
                        @keyframes sync-settings-indeterminate {
                          0%   { transform: translateX(-100%) scaleX(0.4); }
                          50%  { transform: translateX(0%)    scaleX(0.6); }
                          100% { transform: translateX(100%)  scaleX(0.4); }
                        }
                        .sync-settings-bar {
                          animation: sync-settings-indeterminate 1.4s ease-in-out infinite;
                          transform-origin: left center;
                        }
                      `}</style>
                      <div
                        className={isSyncing ? "sync-settings-bar" : undefined}
                        style={{
                          width: isCompleted ? "100%" : "40%",
                          height: "100%",
                          backgroundColor: isError ? "#ef4444" : isCompleted ? "#22c55e" : "var(--color-accent)",
                          borderRadius: "3px",
                          transition: !isSyncing ? "width 0.3s ease" : undefined,
                        }}
                      />
                    </>
                  )}
                </div>
                {liveProgress.total !== undefined && liveProgress.total > 0 && (
                  <p style={{ margin: "5px 0 0", fontSize: "12px", color: "var(--color-text-secondary)" }}>
                    {liveProgress.current.toLocaleString()} / {liveProgress.total.toLocaleString()}
                    {liveProgress.percentage !== undefined && ` (${liveProgress.percentage.toFixed(1)}%)`}
                  </p>
                )}
                {(liveProgress.total === undefined || liveProgress.total === 0) && liveProgress.current > 0 && (
                  <p style={{ margin: "5px 0 0", fontSize: "12px", color: "var(--color-text-secondary)" }}>
                    {t("settings.synced", "Synced")}: {liveProgress.current.toLocaleString()}
                  </p>
                )}
              </div>
            </div>
            {isSyncing && (
              <button
                onClick={handleCancelSync}
                style={{
                  padding: "6px 12px",
                  borderRadius: "6px",
                  border: "1px solid #ef4444",
                  backgroundColor: "transparent",
                  color: "#ef4444",
                  fontSize: "12px",
                  cursor: "pointer",
                  marginLeft: "12px",
                  flexShrink: 0,
                }}
              >
                {t("settings.cancelSync", "Cancel Sync")}
              </button>
            )}
          </div>
        </div>
      )}

      <div style={{ marginBottom: "20px" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
            <input
              type="radio"
              name="sync_strategy"
              value="recent"
              checked={config.syncStrategy === 'recent'}
              onChange={(e) => setConfig({ ...config, syncStrategy: e.target.value as any })}
            />
            <span style={{ fontSize: "14px" }}>
              {t("settings.syncStrategyRecent", "Sync recent messages only (recommended)")}
            </span>
          </label>

          <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
            <input
              type="radio"
              name="sync_strategy"
              value="all"
              checked={config.syncStrategy === 'all'}
              onChange={(e) => setConfig({ ...config, syncStrategy: e.target.value as any })}
            />
            <span style={{ fontSize: "14px" }}>
              {t("settings.syncStrategyAll", "Sync all messages")}
            </span>
          </label>

          <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
            <input
              type="radio"
              name="sync_strategy"
              value="since_date"
              checked={config.syncStrategy === 'since_date'}
              onChange={(e) => setConfig({ ...config, syncStrategy: e.target.value as any, syncSinceDate: '' })}
            />
            <span style={{ fontSize: "14px" }}>
              {t("settings.syncStrategySinceDate", "Sync messages since a specific date")}
            </span>
          </label>

          {config.syncStrategy === 'since_date' && (
            <div style={{ marginLeft: "24px" }}>
              <input
                type="date"
                value={config.syncSinceDate || ''}
                onChange={(e) => setConfig({ ...config, syncSinceDate: e.target.value })}
                max={new Date().toISOString().split('T')[0]}
                style={{ ...inputStyle, width: "200px" }}
              />
            </div>
          )}
        </div>

        {(config.syncStrategy === 'all' || config.syncStrategy === 'since_date') && (
          <div
            style={{
              marginTop: "16px",
              padding: "12px",
              borderRadius: "6px",
              backgroundColor: "rgba(251, 191, 36, 0.1)",
              border: "1px solid rgba(251, 191, 36, 0.3)",
            }}
          >
            <p style={{ margin: 0, fontSize: "13px", color: "#f59e0b", lineHeight: 1.5 }}>
              {t("settings.syncWarning", "⚠️ Syncing large amounts of mail may take a long time and use significant storage. You can cancel at any time; already synced messages will be kept.")}
            </p>
          </div>
        )}
      </div>

      <button
        onClick={handleSave}
        disabled={saving}
        style={{
          padding: "10px 24px",
          borderRadius: "6px",
          border: "none",
          backgroundColor: "var(--color-accent)",
          color: "#fff",
          fontSize: "14px",
          fontWeight: 600,
          cursor: saving ? "not-allowed" : "pointer",
          opacity: saving ? 0.7 : 1,
        }}
      >
        {saving ? t("settings.saving", "Saving...") : t("settings.saveSyncConfig", "Save Settings")}
      </button>
    </div>
  );
}
