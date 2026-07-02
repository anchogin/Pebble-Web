import { useEffect, useMemo, useRef, useState } from "react";
import { Plus, Trash2, Mail, Pencil, Plug, RefreshCw } from "lucide-react";
import ConfirmDialog from "@/components/ConfirmDialog";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { useQueryClient } from "@tanstack/react-query";
import {
  deleteAccount,
  testAccountConnection,
  updateAccount,
  getAccountConfig,
  triggerSync,
} from "@/lib/api";
import type { Account, ConnectionSecurity, AccountConfig } from "@/lib/api";
import { useAccountsQuery, accountsQueryKey } from "@/hooks/queries";
import { useMailStore } from "@/stores/mail.store";
import { useUIStore, type RealtimeStatus } from "@/stores/ui.store";
import { useToastStore } from "@/stores/toast.store";
import AccountSetup from "@/components/AccountSetup";
import { extractErrorMessage } from "@/lib/extractErrorMessage";
import { getSignature, setSignature } from "@/lib/signatures";
import { ACCOUNT_COLOR_PRESETS, assignAccountColors, getAccountColor } from "@/lib/accountColors";
import { inputStyle, labelStyle } from "../../styles/form";
import SyncProgressDialog from "@/components/SyncProgressDialog";
import { wsClient } from "@/lib/websocket";

export default function AccountsTab() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { data: accounts = [] } = useAccountsQuery();
  const accountColorsById = useMemo(() => assignAccountColors(accounts), [accounts]);
  const realtimeStatusByAccount = useUIStore((state) => state.realtimeStatusByAccount);
  const [showSetup, setShowSetup] = useState(false);
  const [editingAccount, setEditingAccount] = useState<Account | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; email: string } | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult] = useState<{ id: string; ok: boolean; message: string } | null>(null);
  const [syncingAccountId, setSyncingAccountId] = useState<string | null>(null);
  const [syncProgress, setSyncProgress] = useState<any>(null);
  const syncingAccountIdRef = useRef<string | null>(null);
  const [syncLogs, setSyncLogs] = useState<Map<string, any[]>>(new Map());
  const syncLogsEndRefs = useRef<Map<string, HTMLDivElement | null>>(new Map());

  // Keep ref in sync with state
  useEffect(() => {
    syncingAccountIdRef.current = syncingAccountId;
  }, [syncingAccountId]);

  useEffect(() => {
    syncLogs.forEach((_, accountId) => {
      const el = syncLogsEndRefs.current.get(accountId);
      el?.scrollIntoView({ behavior: "smooth" });
    });
  }, [syncLogs]);

  // Listen for sync progress updates via WebSocket
  useEffect(() => {
    console.log("[AccountsTab] mounting WS listeners, wsClient=", wsClient);
    wsClient.connect();

    const handleSyncProgress = (msg: any) => {
      const currentAccountId = syncingAccountIdRef.current;
      if (!currentAccountId || msg.account_id !== currentAccountId) return;
      
      setSyncProgress((prev: any) => ({
        ...prev,
        status: msg.status || prev.status,
        phase: msg.phase || prev.phase,
        message: msg.message || prev.message,
        progress: msg.progress || prev.progress,
      }));
    };

    const handleSyncStarted = (msg: any) => {
      const currentAccountId = syncingAccountIdRef.current;
      if (!currentAccountId || msg.account_id !== currentAccountId) return;
      
      setSyncLogs((prev) => {
        const next = new Map(prev);
        next.set(msg.account_id, []);
        return next;
      });

      setSyncProgress({
        accountId: msg.account_id,
        status: "started",
        phase: "initial",
        message: t("settings.syncing", "Syncing mail..."),
        progress: {
          current: 0,
          total: undefined,
          percentage: undefined,
        },
      });
    };

    const handleSyncComplete = (msg: any) => {
      const currentAccountId = syncingAccountIdRef.current;
      if (!currentAccountId || msg.account_id !== currentAccountId) return;
      
      setSyncProgress((prev: any) => ({
        ...prev,
        status: "completed",
        message: t("settings.syncCompleted", "Sync completed"),
      }));
    };

    const handleSyncLog = (msg: any) => {
      console.log("[SyncLog] received:", msg);
      if (!msg.account_id || !msg.log) {
        console.warn("[SyncLog] missing account_id or log, ignoring:", msg);
        return;
      }
      setSyncLogs((prev) => {
        const existing = prev.get(msg.account_id) ?? [];
        const next = new Map(prev);
        next.set(msg.account_id, [...existing.slice(-499), msg.log]);
        console.log("[SyncLog] stored log for", msg.account_id, "total:", existing.length + 1);
        return next;
      });
    };

    wsClient.on("sync_progress", handleSyncProgress);
    wsClient.on("sync_started", handleSyncStarted);
    wsClient.on("sync_complete", handleSyncComplete);
    wsClient.on("sync_log", handleSyncLog);

    return () => {
      wsClient.off("sync_progress", handleSyncProgress);
      wsClient.off("sync_started", handleSyncStarted);
      wsClient.off("sync_complete", handleSyncComplete);
      wsClient.off("sync_log", handleSyncLog);
    };
  }, [t]);

  async function doTestConnection(accountId: string) {
    setTestingId(accountId);
    try {
      const response = await testAccountConnection(accountId);
      useToastStore.getState().addToast({
        message: response.message || t("settings.testConnectionSuccess", "Connection successful"),
        type: "success",
      });
    } catch (err) {
      const msg = extractErrorMessage(err);
      useToastStore.getState().addToast({
        message: t("settings.testConnectionFailed", "Connection failed: {{error}}", { error: msg }),
        type: "error",
      });
    } finally {
      setTestingId(null);
    }
  }

  async function doStartSync(accountId: string) {
    console.log('[Sync] Starting sync for account:', accountId);
    setSyncingAccountId(accountId);
    setSyncProgress({
      accountId,
      status: "started",
      phase: "initial",
      message: t("settings.syncing", "Syncing mail..."),
      progress: {
        current: 0,
        total: undefined,
        percentage: undefined,
      },
    });
    
    try {
      console.log('[Sync] Calling triggerSync API...');
      await triggerSync(accountId, "manual");
      console.log('[Sync] triggerSync API call succeeded');
      useToastStore.getState().addToast({
        message: t("settings.syncStarted", "Sync started"),
        type: "success",
      });
    } catch (err) {
      console.error('[Sync] triggerSync API call failed:', err);
      const msg = extractErrorMessage(err);
      useToastStore.getState().addToast({
        message: t("settings.syncFailed", "Failed to start sync: {{error}}", { error: msg }),
        type: "error",
      });
      setSyncingAccountId(null);
      setSyncProgress(null);
    }
  }

  async function doDelete(accountId: string) {
    try {
      await deleteAccount(accountId);
      if (useMailStore.getState().activeAccountId === accountId) {
        useMailStore.getState().setActiveAccountId(null);
      }
      await queryClient.invalidateQueries({ queryKey: accountsQueryKey });
      useToastStore.getState().addToast({
        message: t("settings.deleteAccountSuccess", "Account removed"),
        type: "success",
      });
    } catch (err) {
      const msg = extractErrorMessage(err);
      useToastStore.getState().addToast({
        message: t("settings.deleteAccountFailed", "Failed to remove account: {{error}}", { error: msg }),
        type: "error",
      });
    }
  }

  return (
    <div>
      {/* Section header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "20px",
        }}
      >
        <h2
          style={{
            margin: 0,
            fontSize: "16px",
            fontWeight: 600,
            color: "var(--color-text-primary)",
          }}
        >
          {t("settings.emailAccounts")}
        </h2>
        <button
          onClick={() => setShowSetup(true)}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "6px",
            padding: "7px 14px",
            borderRadius: "6px",
            border: "none",
            backgroundColor: "var(--color-accent)",
            color: "#fff",
            fontSize: "13px",
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
          <Plus size={14} />
          {t("settings.addAccount")}
        </button>
      </div>

      {/* Empty state */}
      {accounts.length === 0 ? (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: "12px",
            padding: "48px 0",
            color: "var(--color-text-secondary)",
          }}
        >
          <Mail size={40} strokeWidth={1.5} />
          <p style={{ margin: 0, fontSize: "14px" }}>{t("settings.noAccounts")}</p>
          <button
            onClick={() => setShowSetup(true)}
            style={{
              marginTop: "4px",
              padding: "8px 18px",
              borderRadius: "6px",
              border: "1px solid var(--color-border)",
              backgroundColor: "transparent",
              color: "var(--color-text-primary)",
              fontSize: "13px",
              cursor: "pointer",
            }}
          >
            {t("settings.addFirstAccount")}
          </button>
        </div>
      ) : (
        /* Account list */
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "1px",
            borderRadius: "8px",
            overflow: "hidden",
            border: "1px solid var(--color-border)",
          }}
        >
          {accounts.map((account, index) => {
            const realtimeStatus = realtimeStatusByAccount[account.id];
            const realtimeLabel = getAccountRealtimeStatusText(realtimeStatus, t);
            const accountColor = accountColorsById.get(account.id) ?? getAccountColor(account);
            const accountLogs = syncLogs.get(account.id) ?? [];
            const isSyncingThis = syncingAccountId === account.id;
            const syncStatus = isSyncingThis ? (syncProgress?.status ?? null) : null;

            return (
              <div key={account.id}>
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "14px 16px",
                    backgroundColor: "var(--color-bg)",
                    borderTop: index > 0 ? "1px solid var(--color-border)" : "none",
                  }}
                >
                <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "8px", minWidth: 0 }}>
                    <span
                      aria-hidden="true"
                      style={{
                        width: "8px",
                        height: "8px",
                        borderRadius: "50%",
                        backgroundColor: accountColor,
                        flexShrink: 0,
                      }}
                    />
                    <span style={{ fontSize: "13px", fontWeight: 500 }}>
                      {account.display_name}
                    </span>
                  </div>
                  <span
                    style={{
                      fontSize: "12px",
                      color: "var(--color-text-secondary)",
                    }}
                  >
                    {account.email}
                  </span>
                  <span
                    style={{
                      fontSize: "11px",
                      color: "var(--color-text-secondary)",
                      textTransform: "capitalize",
                    }}
                  >
                    {account.provider}
                  </span>
                  {realtimeLabel && (
                    <span
                      aria-label={realtimeLabel}
                      title={realtimeStatus?.message ?? realtimeLabel}
                      style={{
                        fontSize: "11px",
                        color: "var(--color-text-secondary)",
                      }}
                    >
                      {realtimeLabel}
                    </span>
                  )}
                </div>
                <div style={{ display: "flex", gap: "4px", alignItems: "center" }}>
                  <button
                    onClick={() => doStartSync(account.id)}
                    disabled={syncingAccountId === account.id || testingId === account.id}
                    title={t("settings.syncMail", "Sync Mail")}
                    aria-label={t("settings.syncMail", "Sync Mail")}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      padding: "6px",
                      borderRadius: "6px",
                      border: "none",
                      backgroundColor: "transparent",
                      color: syncingAccountId === account.id ? "var(--color-accent)" : "var(--color-text-secondary)",
                      cursor: syncingAccountId === account.id || testingId === account.id ? "not-allowed" : "pointer",
                      opacity: syncingAccountId === account.id || testingId === account.id ? 0.6 : 1,
                    }}
                    onMouseEnter={(e) => {
                      if (syncingAccountId !== account.id && testingId !== account.id) {
                        e.currentTarget.style.color = "var(--color-accent)";
                        e.currentTarget.style.backgroundColor = "var(--color-bg-hover)";
                      }
                    }}
                    onMouseLeave={(e) => {
                      if (syncingAccountId !== account.id && testingId !== account.id) {
                        e.currentTarget.style.color = "var(--color-text-secondary)";
                        e.currentTarget.style.backgroundColor = "transparent";
                      }
                    }}
                  >
                    <RefreshCw size={15} />
                  </button>
                  <button
                    onClick={() => doTestConnection(account.id)}
                    disabled={testingId === account.id}
                    title={t("accountSetup.testConnection", "Test Connection")}
                    aria-label={t("accountSetup.testConnection", "Test Connection")}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      padding: "6px",
                      borderRadius: "6px",
                      border: "none",
                      backgroundColor: "transparent",
                      color: testingId === account.id ? "var(--color-accent)" : "var(--color-text-secondary)",
                      cursor: testingId === account.id ? "wait" : "pointer",
                      opacity: testingId === account.id ? 0.6 : 1,
                    }}
                    onMouseEnter={(e) => {
                      if (testingId !== account.id) {
                        e.currentTarget.style.color = "var(--color-accent)";
                        e.currentTarget.style.backgroundColor = "var(--color-bg-hover)";
                      }
                    }}
                    onMouseLeave={(e) => {
                      if (testingId !== account.id) {
                        e.currentTarget.style.color = "var(--color-text-secondary)";
                        e.currentTarget.style.backgroundColor = "transparent";
                      }
                    }}
                  >
                    <Plug size={15} />
                  </button>
                  <button
                    onClick={() => setEditingAccount(account)}
                    title={t("settings.editAccount", "Edit account")}
                    aria-label={t("settings.editAccount", "Edit account")}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      padding: "6px",
                      borderRadius: "6px",
                      border: "none",
                      backgroundColor: "transparent",
                      color: "var(--color-text-secondary)",
                      cursor: "pointer",
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.color = "var(--color-accent)";
                      e.currentTarget.style.backgroundColor = "var(--color-bg-hover)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.color = "var(--color-text-secondary)";
                      e.currentTarget.style.backgroundColor = "transparent";
                    }}
                  >
                    <Pencil size={15} />
                  </button>
                  <button
                    onClick={() => setDeleteTarget({ id: account.id, email: account.email })}
                    title={t("settings.removeAccount")}
                    aria-label={t("settings.removeAccount")}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      padding: "6px",
                      borderRadius: "6px",
                      border: "none",
                      backgroundColor: "transparent",
                      color: "var(--color-text-secondary)",
                      cursor: "pointer",
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.color = "#ef4444";
                      e.currentTarget.style.backgroundColor = "rgba(239,68,68,0.08)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.color = "var(--color-text-secondary)";
                      e.currentTarget.style.backgroundColor = "transparent";
                    }}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>
              {/* Inline sync log panel */}
              {accountLogs.length > 0 && (
                <div
                  style={{
                    borderTop: "1px solid var(--color-border)",
                    padding: "0",
                    backgroundColor: "var(--color-bg-secondary)",
                  }}
                >
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      padding: "6px 16px",
                      borderBottom: "1px solid var(--color-border)",
                    }}
                  >
                    <span
                      style={{
                        fontSize: "11px",
                        fontWeight: 600,
                        color: "var(--color-text-secondary)",
                        textTransform: "uppercase",
                        letterSpacing: "0.05em",
                      }}
                    >
                      {syncStatus === "completed"
                        ? t("settings.syncLog", "Sync Log") + " ✓"
                        : syncStatus === "error"
                        ? t("settings.syncLog", "Sync Log") + " ✗"
                        : isSyncingThis
                        ? t("settings.syncLog", "Sync Log") + " ●"
                        : t("settings.syncLog", "Sync Log")}
                    </span>
                    <button
                      onClick={() =>
                        setSyncLogs((prev) => {
                          const next = new Map(prev);
                          next.delete(account.id);
                          return next;
                        })
                      }
                      style={{
                        background: "none",
                        border: "none",
                        cursor: "pointer",
                        fontSize: "11px",
                        color: "var(--color-text-secondary)",
                        padding: "2px 6px",
                      }}
                    >
                      {t("common.clear", "Clear")}
                    </button>
                  </div>
                  <div
                    style={{
                      overflowY: "auto",
                      maxHeight: "220px",
                      padding: "8px 16px",
                      fontFamily: "monospace",
                      fontSize: "11px",
                      lineHeight: 1.7,
                    }}
                  >
                    {accountLogs.map((entry: any, i: number) => (
                      <div
                        key={i}
                        style={{
                          color:
                            entry.error
                              ? "#ef4444"
                              : entry.level === "warn"
                              ? "#f59e0b"
                              : "var(--color-text-primary)",
                          wordBreak: "break-all",
                          paddingBottom: "1px",
                        }}
                      >
                        <span style={{ opacity: 0.45, marginRight: "6px" }}>
                          {new Date(entry.timestamp * 1000).toLocaleTimeString([], {
                            hour: "2-digit",
                            minute: "2-digit",
                            second: "2-digit",
                          })}
                        </span>
                        <span
                          style={{
                            color: "var(--color-accent)",
                            fontWeight: 600,
                            marginRight: "4px",
                          }}
                        >
                          [{entry.action}]
                        </span>
                        {entry.server && (
                          <span style={{ opacity: 0.65, marginRight: "6px" }}>
                            {entry.server}
                          </span>
                        )}
                        {entry.request && (
                          <span style={{ opacity: 0.8 }}>{entry.request}</span>
                        )}
                        {entry.response && !entry.error && (
                          <span style={{ color: "#22c55e", marginLeft: "4px" }}>
                            → {entry.response}
                          </span>
                        )}
                        {entry.message_count !== undefined && !entry.error && (
                          <span style={{ opacity: 0.55, marginLeft: "4px" }}>
                            ({entry.message_count} msgs)
                          </span>
                        )}
                        {entry.error && (
                          <span style={{ color: "#ef4444", marginLeft: "4px" }}>
                            ✗ {entry.error}
                          </span>
                        )}
                      </div>
                    ))}
                    <div
                      ref={(el) => void syncLogsEndRefs.current.set(account.id, el)}
                    />
                  </div>
                </div>
              )}
            </div>
            );
          })}
        </div>
      )}

      {/* Test result */}
      {testResult && (
        <div
          style={{
            marginTop: "10px",
            padding: "10px 12px",
            borderRadius: "6px",
            backgroundColor: testResult.ok ? "rgba(34,197,94,0.1)" : "rgba(239,68,68,0.1)",
            border: `1px solid ${testResult.ok ? "rgba(34,197,94,0.3)" : "rgba(239,68,68,0.3)"}`,
            color: testResult.ok ? "#22c55e" : "#ef4444",
            fontSize: "12px",
            whiteSpace: "pre-wrap",
            fontFamily: "monospace",
            lineHeight: 1.5,
          }}
        >
          {testResult.message}
        </div>
      )}

      {/* Delete confirmation */}
      {deleteTarget && (
        <ConfirmDialog
          title={t("settings.removeAccount", "Remove Account")}
          message={t("settings.confirmDeleteAccount", { email: deleteTarget.email })}
          destructive
          onCancel={() => setDeleteTarget(null)}
          onConfirm={() => {
            doDelete(deleteTarget.id);
            setDeleteTarget(null);
          }}
        />
      )}

      {/* AccountSetup modal */}
      {showSetup && <AccountSetup onClose={() => setShowSetup(false)} />}

      {/* Edit account modal */}
      {editingAccount && (
        <EditAccountModal
          account={editingAccount}
          initialColor={accountColorsById.get(editingAccount.id) ?? getAccountColor(editingAccount)}
          onClose={() => setEditingAccount(null)}
          onSaved={async () => {
            setEditingAccount(null);
            await queryClient.invalidateQueries({ queryKey: accountsQueryKey });
          }}
        />
      )}

      {/* Sync progress dialog */}
      {syncProgress && syncingAccountId && (
        <SyncProgressDialog
          accountId={syncingAccountId}
          progress={syncProgress}
          onClose={() => {
            setSyncingAccountId(null);
            setSyncProgress(null);
          }}
        />
      )}
    </div>
  );
}

function getAccountRealtimeStatusText(
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

function EditAccountModal({ account, initialColor, onClose, onSaved }: {
  account: Account;
  initialColor: string;
  onClose: () => void;
  onSaved: () => void;
}) {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement>(null);
  const emailInputRef = useRef<HTMLInputElement>(null);
  const [displayName, setDisplayName] = useState(account.display_name);
  const [email, setEmail] = useState(account.email);
  const [accountColor, setAccountColor] = useState(initialColor);
  const [password, setPassword] = useState("");
  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState("");
  const [smtpHost, setSmtpHost] = useState("");
  const [smtpPort, setSmtpPort] = useState("");
  const [imapSecurity, setImapSecurity] = useState<ConnectionSecurity | "">("");
  const [smtpSecurity, setSmtpSecurity] = useState<ConnectionSecurity | "">("");
  const [signature, setSignatureValue] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [syncStrategy, setSyncStrategy] = useState<'recent' | 'all' | 'since_date'>('recent');
  const [syncSinceDate, setSyncSinceDate] = useState<string>("");
  const isOAuth = account.provider === "gmail" || account.provider === "outlook";

  useEffect(() => {
    let cancelled = false;
    getSignature(account.id)
      .then((loaded) => {
        if (!cancelled) setSignatureValue(loaded);
      })
      .catch((err) => {
        console.warn("Failed to load signature:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [account.id]);

  // Load account config for editing
  useEffect(() => {
    let cancelled = false;
    getAccountConfig(account.id)
      .then((config: AccountConfig) => {
        if (!cancelled) {
          setDisplayName(config.display_name);
          setEmail(config.email);
          if (config.color) setAccountColor(config.color);
          if (config.imap_host) setImapHost(config.imap_host);
          if (config.imap_port) setImapPort(String(config.imap_port));
          if (config.imap_security) setImapSecurity(config.imap_security);
          if (config.smtp_host) setSmtpHost(config.smtp_host);
          if (config.smtp_port) setSmtpPort(String(config.smtp_port));
          if (config.smtp_security) setSmtpSecurity(config.smtp_security);
        }
      })
      .catch((err) => {
        console.warn("Failed to load account config:", err);
      });
    return () => {
      cancelled = true;
    };
  }, [account.id]);

  // Initialize sync config from account data
  useEffect(() => {
    const syncState = account.sync_state || {};
    setSyncStrategy(syncState.sync_strategy || 'recent');
    setSyncSinceDate(syncState.sync_since_date || "");
  }, [account.id, account.sync_state]);

  useEffect(() => {
    const previousFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;

    emailInputRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key === "Tab" && dialogRef.current) {
        const focusable = dialogRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
        );
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    }

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [onClose]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      if (isOAuth) {
        await updateAccount(
          account.id,
          email,
          displayName,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          accountColor,
        );
      } else {
        await updateAccount(
          account.id,
          email,
          displayName,
          password || undefined,
          imapHost || undefined,
          imapPort ? parseInt(imapPort, 10) : undefined,
          smtpHost || undefined,
          smtpPort ? parseInt(smtpPort, 10) : undefined,
          imapSecurity || undefined,
          smtpSecurity || undefined,
          undefined,
          undefined,
          accountColor,
        );
        
        // Update sync strategy if changed
        const currentStrategy = account.sync_state?.sync_strategy || 'recent';
        const currentSinceDate = account.sync_state?.sync_since_date || "";
        if (syncStrategy !== currentStrategy || syncSinceDate !== currentSinceDate) {
          const { updateSyncConfig } = await import("@/lib/api");
          await updateSyncConfig(account.id, {
            syncStrategy,
            syncSinceDate: syncStrategy === 'since_date' ? syncSinceDate : undefined,
          });
        }
      }
      await setSignature(account.id, signature);
      onSaved();
    } catch (err) {
      setError(extractErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  const fieldStyle: React.CSSProperties = {
    display: "flex",
    flexDirection: "column",
  };
  const colorInputValue = /^#[0-9a-fA-F]{6}$/.test(accountColor) ? accountColor : initialColor;
  return (
    <div
      ref={dialogRef}
      role="dialog"
      aria-modal="true"
      aria-labelledby="edit-account-title"
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
      <div
        style={{
          width: "480px",
          backgroundColor: "var(--color-bg)",
          borderRadius: "10px",
          boxShadow: "0 20px 60px rgba(0,0,0,0.3)",
          display: "flex",
          flexDirection: "column",
          maxHeight: "90vh",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "16px 20px",
            borderBottom: "1px solid var(--color-border)",
          }}
        >
          <h2 id="edit-account-title" style={{ margin: 0, fontSize: "15px", fontWeight: 600, color: "var(--color-text-primary)" }}>
            {t("settings.editAccount", "Edit Account")}
          </h2>
          <button
            onClick={onClose}
            aria-label={t("common.close")}
            style={{ backgroundColor: "transparent", backgroundImage: "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='18' height='18' viewBox='0 0 24 24' fill='none' stroke='%236b7280' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M18 6 6 18'/%3E%3Cpath d='m6 6 12 12'/%3E%3C/svg%3E\")", backgroundPosition: "center", backgroundRepeat: "no-repeat", backgroundSize: "18px 18px", border: "none", cursor: "pointer", padding: "4px", borderRadius: "4px", color: "var(--color-text-secondary)", display: "flex", fontSize: 0 }}
          >
            ×
          </button>
        </div>

        <div className="scroll-region edit-account-scroll" style={{ overflowY: "auto", padding: "20px" }}>
          <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
            <div style={fieldStyle}>
              <label style={labelStyle}>{t("accountSetup.displayName")}</label>
              <input aria-label={t("accountSetup.displayName")} style={inputStyle} type="text" required value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
            </div>
            <div style={fieldStyle}>
              <label style={labelStyle}>{t("accountSetup.emailAddress")}</label>
              <input aria-label={t("accountSetup.emailAddress")} ref={emailInputRef} style={inputStyle} type="email" required value={email} onChange={(e) => setEmail(e.target.value)} />
            </div>
            <div style={fieldStyle}>
              <label htmlFor="account-color" style={labelStyle}>{t("settings.accountColor", "Account color")}</label>
              <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
                <input
                  id="account-color"
                  aria-label={t("settings.accountColor", "Account color")}
                  type="color"
                  value={colorInputValue}
                  onChange={(e) => setAccountColor(e.target.value)}
                  style={{
                    width: "38px",
                    height: "32px",
                    padding: "2px",
                    border: "1px solid var(--color-border)",
                    borderRadius: "6px",
                    backgroundColor: "var(--color-bg)",
                    cursor: "pointer",
                  }}
                />
                <input
                  aria-label={t("settings.accountColorHex", "Account color hex")}
                  style={{ ...inputStyle, width: "96px", fontFamily: "monospace" }}
                  value={accountColor}
                  onChange={(e) => {
                    const value = e.target.value;
                    setAccountColor(value.startsWith("#") ? value : `#${value}`);
                  }}
                  pattern="^#[0-9a-fA-F]{6}$"
                  maxLength={7}
                />
              </div>
              <div
                aria-label={t("settings.accountColorPresets", "Color presets")}
                role="group"
                style={{
                  display: "flex",
                  flexWrap: "wrap",
                  gap: "6px",
                  marginTop: "8px",
                }}
              >
                {ACCOUNT_COLOR_PRESETS.map((preset) => {
                  const presetLabel = `${t("settings.useAccountColorPreset", "Use color")} ${preset.color}`;
                  const selected = accountColor.toLowerCase() === preset.color;
                  return (
                    <button
                      key={preset.color}
                      type="button"
                      aria-label={presetLabel}
                      aria-pressed={selected}
                      title={presetLabel}
                      onClick={() => setAccountColor(preset.color)}
                      style={{
                        width: "22px",
                        height: "22px",
                        borderRadius: "50%",
                        border: selected ? "2px solid var(--color-text-primary)" : "1px solid var(--color-border)",
                        backgroundColor: preset.color,
                        cursor: "pointer",
                        padding: 0,
                        boxShadow: selected ? `0 0 0 2px ${preset.color}33` : "none",
                      }}
                    />
                  );
                })}
              </div>
            </div>
            {isOAuth ? (
              <div
                style={{
                  padding: "10px 12px",
                  borderRadius: "6px",
                  backgroundColor: "rgba(59,130,246,0.08)",
                  border: "1px solid rgba(59,130,246,0.25)",
                  color: "var(--color-text-secondary)",
                  fontSize: "12px",
                  lineHeight: 1.5,
                }}
              >
                {t(
                  "settings.oauthAccountNote",
                  "This account uses OAuth. Provider sign-in, password, and IMAP/SMTP settings are managed by the provider."
                )}
              </div>
            ) : (
              <>
                <div style={fieldStyle}>
                  <label style={labelStyle}>{t("accountSetup.password")} <span style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}>({t("settings.leaveEmptyKeep", "leave empty to keep current")})</span></label>
                  <input aria-label={t("accountSetup.password")} style={inputStyle} type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
                </div>

                <div style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: "12px" }}>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>{t("accountSetup.imapHost")} <span style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}>({t("settings.optional", "optional")})</span></label>
                    <input aria-label={t("accountSetup.imapHost")} style={inputStyle} type="text" value={imapHost} onChange={(e) => setImapHost(e.target.value)} placeholder={t("settings.leaveEmptyKeep")} />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>{t("accountSetup.imapPort")}</label>
                    <input aria-label={t("accountSetup.imapPort")} style={{ ...inputStyle, width: "70px" }} type="number" value={imapPort} onChange={(e) => setImapPort(e.target.value)} />
                  </div>
                  <div style={fieldStyle}>
                    <label htmlFor="accountsetup-imap-security" style={labelStyle}>{t("accountSetup.security", "Security")}</label>
                    <select id="accountsetup-imap-security" value={imapSecurity} onChange={(e) => setImapSecurity(e.target.value as ConnectionSecurity | "")} style={{ ...inputStyle, width: "110px" }}>
                      <option value="">{t("settings.leaveEmptyKeep", "keep current")}</option>
                      <option value="tls">{t("accountSetup.securityTls", "SSL/TLS")}</option>
                      <option value="starttls">{t("accountSetup.securityStarttls", "STARTTLS")}</option>
                    </select>
                  </div>
                </div>

                <div style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: "12px" }}>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>{t("accountSetup.smtpHost")} <span style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}>({t("settings.optional", "optional")})</span></label>
                    <input aria-label={t("accountSetup.smtpHost")} style={inputStyle} type="text" value={smtpHost} onChange={(e) => setSmtpHost(e.target.value)} placeholder={t("settings.leaveEmptyKeep")} />
                  </div>
                  <div style={fieldStyle}>
                    <label style={labelStyle}>{t("accountSetup.smtpPort")}</label>
                    <input aria-label={t("accountSetup.smtpPort")} style={{ ...inputStyle, width: "70px" }} type="number" value={smtpPort} onChange={(e) => setSmtpPort(e.target.value)} />
                  </div>
                  <div style={fieldStyle}>
                    <label htmlFor="accountsetup-smtp-security" style={labelStyle}>{t("accountSetup.security", "Security")}</label>
                    <select id="accountsetup-smtp-security" value={smtpSecurity} onChange={(e) => setSmtpSecurity(e.target.value as ConnectionSecurity | "")} style={{ ...inputStyle, width: "110px" }}>
                      <option value="">{t("settings.leaveEmptyKeep", "keep current")}</option>
                      <option value="tls">{t("accountSetup.securityTls", "SSL/TLS")}</option>
                      <option value="starttls">{t("accountSetup.securityStarttls", "STARTTLS")}</option>
                    </select>
                  </div>
                </div>

              </>
            )}

            {/* Signature */}
            <div style={fieldStyle}>
              <label style={labelStyle}>{t("settings.signature", "Signature")} <span style={{ color: "var(--color-text-secondary)", fontWeight: 400 }}>({t("settings.optional", "optional")})</span></label>
              <textarea
                style={{ ...inputStyle, minHeight: "80px", resize: "vertical", fontFamily: "inherit" }}
                value={signature}
                onChange={(e) => setSignatureValue(e.target.value)}
                placeholder={t("settings.signaturePlaceholder", "Your email signature...")}
              />
            </div>

            {/* Sync Strategy */}
            {!isOAuth && (
              <div style={{ ...fieldStyle, marginTop: "8px" }}>
                <label style={labelStyle}>{t("settings.syncStrategy", "Sync Strategy")}</label>
                <div style={{ display: "flex", flexDirection: "column", gap: "10px", marginTop: "10px" }}>
                  <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
                    <input
                      type="radio"
                      name="sync_strategy"
                      value="recent"
                      checked={syncStrategy === 'recent'}
                      onChange={(e) => setSyncStrategy(e.target.value as any)}
                    />
                    <span style={{ fontSize: "13px" }}>{t("settings.syncStrategyRecent", "Sync recent messages only (recommended)")}</span>
                  </label>

                  <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
                    <input
                      type="radio"
                      name="sync_strategy"
                      value="all"
                      checked={syncStrategy === 'all'}
                      onChange={(e) => setSyncStrategy(e.target.value as any)}
                    />
                    <span style={{ fontSize: "13px" }}>{t("settings.syncStrategyAll", "Sync all messages")}</span>
                  </label>

                  <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
                    <input
                      type="radio"
                      name="sync_strategy"
                      value="since_date"
                      checked={syncStrategy === 'since_date'}
                      onChange={(e) => setSyncStrategy(e.target.value as any)}
                    />
                    <span style={{ fontSize: "13px" }}>{t("settings.syncStrategySinceDate", "Sync messages since a specific date")}</span>
                  </label>

                  {syncStrategy === 'since_date' && (
                    <div style={{ marginLeft: "24px" }}>
                      <input
                        type="date"
                        value={syncSinceDate}
                        onChange={(e) => setSyncSinceDate(e.target.value)}
                        max={new Date().toISOString().split('T')[0]}
                        style={{
                          ...inputStyle,
                          width: "200px",
                          padding: "6px 10px",
                          fontSize: "13px",
                        }}
                      />
                    </div>
                  )}
                </div>

                {/* Warning message for full/date sync */}
                {(syncStrategy === 'all' || syncStrategy === 'since_date') && (
                  <div
                    style={{
                      marginTop: "12px",
                      padding: "10px 12px",
                      borderRadius: "6px",
                      backgroundColor: "rgba(251, 191, 36, 0.1)",
                      border: "1px solid rgba(251, 191, 36, 0.3)",
                    }}
                  >
                    <p style={{ margin: 0, fontSize: "12px", color: "#f59e0b", lineHeight: 1.5 }}>
                      {t("settings.syncWarning", "⚠️ Syncing large amounts of mail may take a long time and use significant storage. You can cancel at any time; already synced messages will be kept.")}
                    </p>
                  </div>
                )}
              </div>
            )}

            {error && (
              <div role="alert" aria-live="assertive" style={{ padding: "10px 12px", borderRadius: "6px", backgroundColor: "rgba(239,68,68,0.1)", border: "1px solid rgba(239,68,68,0.3)", color: "#ef4444", fontSize: "13px" }}>
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              style={{
                padding: "9px 16px",
                borderRadius: "6px",
                border: "none",
                backgroundColor: "var(--color-accent)",
                color: "#fff",
                fontSize: "13px",
                fontWeight: 600,
                cursor: loading ? "not-allowed" : "pointer",
                opacity: loading ? 0.7 : 1,
                marginTop: "4px",
              }}
            >
              {loading ? t("common.saving") : t("common.save")}
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
