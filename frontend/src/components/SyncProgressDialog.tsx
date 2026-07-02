import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { X, CheckCircle, XCircle } from "lucide-react";
import { cancelSync, type SyncProgress } from "@/lib/api";
import { useToastStore } from "@/stores/toast.store";
import { wsClient } from "@/lib/websocket";

interface SyncLogEntry {
  timestamp: number;
  level: string;
  server: string;
  action: string;
  request?: string;
  response?: string;
  error?: string;
  message_count?: number;
}

interface Props {
  accountId: string;
  progress: SyncProgress;
  onClose: () => void;
}

export default function SyncProgressDialog({ accountId, progress, onClose }: Props) {
  const { t } = useTranslation();
  const [cancelling, setCancelling] = useState(false);
  const [logs, setLogs] = useState<SyncLogEntry[]>([]);
  const logsEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleLog = (msg: any) => {
      if (msg.account_id !== accountId) return;
      setLogs((prev) => [...prev.slice(-199), msg.log as SyncLogEntry]);
    };
    wsClient.on("sync_log", handleLog);
    return () => wsClient.off("sync_log", handleLog);
  }, [accountId]);

  useEffect(() => {
    logsEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs]);

  const handleCancel = async () => {
    setCancelling(true);
    try {
      await cancelSync(accountId);
      useToastStore.getState().addToast({
        message: t("settings.syncCancelled", "Sync cancelled"),
        type: "info",
      });
    } catch (err) {
      useToastStore.getState().addToast({
        message: t("settings.cancelSyncFailed", "Failed to cancel sync"),
        type: "error",
      });
    } finally {
      setCancelling(false);
    }
  };

  const progressDetail = progress.progress;
  const current = progressDetail?.current ?? 0;
  const total = progressDetail?.total;
  const percentage = progressDetail?.percentage;

  const isCompleted = progress.status === "completed";
  const isError = progress.status === "error";
  const isDone = isCompleted || isError;
  const isRunning = !isDone;
  const hasExactProgress = total !== undefined && total > 0;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="sync-progress-title"
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
    >
      <style>{`
        @keyframes indeterminate {
          0%   { transform: translateX(-100%) scaleX(0.4); }
          50%  { transform: translateX(0%)    scaleX(0.6); }
          100% { transform: translateX(100%)  scaleX(0.4); }
        }
        .sync-progress-indeterminate {
          animation: indeterminate 1.4s ease-in-out infinite;
          transform-origin: left center;
        }
      `}</style>
      <div
        style={{
          width: "min(480px, calc(100vw - 32px))",
          backgroundColor: "var(--color-bg)",
          borderRadius: "10px",
          boxShadow: "0 20px 60px rgba(0,0,0,0.3)",
          padding: "20px",
          display: "flex",
          flexDirection: "column",
          gap: "16px",
          maxHeight: "80vh",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            {isCompleted && <CheckCircle size={18} color="#22c55e" />}
            {isError && <XCircle size={18} color="#ef4444" />}
            <h2
              id="sync-progress-title"
              style={{
                margin: 0,
                fontSize: "15px",
                fontWeight: 600,
                color: isCompleted
                  ? "#22c55e"
                  : isError
                  ? "#ef4444"
                  : "var(--color-text-primary)",
              }}
            >
              {isCompleted
                ? t("settings.syncCompleted", "Sync completed")
                : isError
                ? t("settings.syncFailed", "Sync failed")
                : t("settings.syncing", "Syncing mail...")}
            </h2>
          </div>
          <button
            onClick={onClose}
            aria-label={t("common.close", "Close")}
            style={{
              background: "none",
              border: "none",
              cursor: "pointer",
              padding: "4px",
              borderRadius: "4px",
              color: "var(--color-text-secondary)",
              display: "flex",
              alignItems: "center",
            }}
          >
            <X size={18} />
          </button>
        </div>

        {progress.message && (
          <p style={{ margin: 0, fontSize: "13px", color: "var(--color-text-secondary)" }}>
            {progress.message}
          </p>
        )}

        <div>
          <div
            style={{
              width: "100%",
              height: "8px",
              backgroundColor: "var(--color-border)",
              borderRadius: "4px",
              overflow: "hidden",
            }}
          >
            {hasExactProgress ? (
              <div
                style={{
                  width: `${percentage ?? 0}%`,
                  height: "100%",
                  backgroundColor: isError ? "#ef4444" : "var(--color-accent)",
                  borderRadius: "4px",
                  transition: "width 0.3s ease",
                }}
              />
            ) : (
              <div
                className={isRunning ? "sync-progress-indeterminate" : undefined}
                style={{
                  width: isCompleted ? "100%" : "40%",
                  height: "100%",
                  backgroundColor: isError
                    ? "#ef4444"
                    : isCompleted
                    ? "#22c55e"
                    : "var(--color-accent)",
                  borderRadius: "4px",
                  transition: isDone ? "width 0.3s ease" : undefined,
                }}
              />
            )}
          </div>
          {hasExactProgress && (
            <p
              style={{
                margin: "6px 0 0",
                fontSize: "12px",
                color: "var(--color-text-secondary)",
                textAlign: "center",
              }}
            >
              {current.toLocaleString()} / {total!.toLocaleString()} ({percentage?.toFixed(1)}%)
            </p>
          )}
          {!hasExactProgress && current > 0 && (
            <p
              style={{
                margin: "6px 0 0",
                fontSize: "12px",
                color: "var(--color-text-secondary)",
                textAlign: "center",
              }}
            >
              {t("settings.synced", "Synced")}: {current.toLocaleString()}
            </p>
          )}
        </div>

        {logs.length > 0 && (
          <div
            style={{
              flex: 1,
              overflow: "hidden",
              display: "flex",
              flexDirection: "column",
              gap: "4px",
            }}
          >
            <p
              style={{
                margin: 0,
                fontSize: "11px",
                fontWeight: 600,
                color: "var(--color-text-secondary)",
                textTransform: "uppercase",
                letterSpacing: "0.05em",
              }}
            >
              {t("settings.syncLog", "Server Log")}
            </p>
            <div
              style={{
                overflowY: "auto",
                maxHeight: "160px",
                padding: "8px",
                backgroundColor: "var(--color-bg-secondary)",
                borderRadius: "6px",
                fontFamily: "monospace",
                fontSize: "11px",
                lineHeight: 1.6,
              }}
            >
              {logs.map((entry, i) => (
                <div
                  key={i}
                  style={{
                    color:
                      entry.level === "error"
                        ? "#ef4444"
                        : entry.level === "warn"
                        ? "#f59e0b"
                        : "var(--color-text-secondary)",
                    wordBreak: "break-all",
                  }}
                >
                  <span style={{ opacity: 0.5 }}>
                    {new Date(entry.timestamp * 1000).toLocaleTimeString()}
                  </span>{" "}
                  <span style={{ color: "var(--color-accent)", fontWeight: 600 }}>
                    {entry.server}
                  </span>{" "}
                  <span>{entry.action}</span>
                  {entry.request && (
                    <span style={{ opacity: 0.7 }}> → {entry.request}</span>
                  )}
                  {entry.error && (
                    <span style={{ color: "#ef4444" }}> ✗ {entry.error}</span>
                  )}
                  {entry.message_count !== undefined && (
                    <span style={{ opacity: 0.7 }}> ({entry.message_count} msgs)</span>
                  )}
                </div>
              ))}
              <div ref={logsEndRef} />
            </div>
          </div>
        )}

        <button
          onClick={isDone ? onClose : handleCancel}
          disabled={cancelling}
          style={{
            width: "100%",
            padding: "10px 16px",
            borderRadius: "6px",
            border: isDone
              ? "1px solid var(--color-border)"
              : "1px solid #ef4444",
            backgroundColor: isDone ? "var(--color-bg-hover)" : "transparent",
            color: isDone ? "var(--color-text-primary)" : "#ef4444",
            fontSize: "13px",
            fontWeight: 500,
            cursor: cancelling ? "not-allowed" : "pointer",
            opacity: cancelling ? 0.5 : 1,
          }}
        >
          {isDone
            ? t("common.close", "Close")
            : cancelling
            ? t("settings.cancelling", "Cancelling...")
            : t("settings.cancelSync", "Cancel Sync")}
        </button>
      </div>
    </div>
  );
}
