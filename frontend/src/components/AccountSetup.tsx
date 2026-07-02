import { useEffect, useRef, useState } from "react";
import i18n from "@/lib/i18n";
import { useConfirmStore } from "@/stores/confirm.store";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { addAccount, startSync, testImapConnection, startGoogleOAuth, pollGoogleOAuth } from "@/lib/api";
import type { AddAccountRequest } from "@/lib/api";
import { accountsQueryKey } from "@/hooks/queries";
import { extractErrorMessage } from "@/lib/extractErrorMessage";
import { realtimePreferenceToPollInterval, useUIStore } from "@/stores/ui.store";
import { useToastStore } from "@/stores/toast.store";
import { inputStyle, labelStyle } from "../styles/form";

const FOLDER_REFRESH_ATTEMPTS = 5;
const FOLDER_REFRESH_INTERVAL_MS = 2000;

function refreshFoldersAfterSyncStart(queryClient: QueryClient, accountId: string) {
  void queryClient.invalidateQueries({ queryKey: ["folders", accountId] });
  void queryClient.invalidateQueries({ queryKey: ["folders"] });

  const pollFolders = (attempts: number) => {
    if (attempts <= 0) return;
    window.setTimeout(() => {
      void queryClient.invalidateQueries({ queryKey: ["folders"] });
      void queryClient.invalidateQueries({ queryKey: ["folders", accountId] });
      pollFolders(attempts - 1);
    }, FOLDER_REFRESH_INTERVAL_MS);
  };
  pollFolders(FOLDER_REFRESH_ATTEMPTS);
}

const PRESETS: Record<
  string,
  Pick<
    AddAccountRequest,
    "imap_host" | "imap_port" | "smtp_host" | "smtp_port" | "imap_security" | "smtp_security"
  >
> = {
  gmail: {
    imap_host: "imap.gmail.com",
    imap_port: 993,
    imap_security: "tls",
    smtp_host: "smtp.gmail.com",
    smtp_port: 587,
    smtp_security: "starttls",
  },
  outlook: {
    imap_host: "outlook.office365.com",
    imap_port: 993,
    imap_security: "tls",
    smtp_host: "smtp.office365.com",
    smtp_port: 587,
    smtp_security: "starttls",
  },
  qq: {
    imap_host: "imap.qq.com",
    imap_port: 993,
    imap_security: "tls",
    smtp_host: "smtp.qq.com",
    smtp_port: 465,
    smtp_security: "tls",
  },
  "163": {
    imap_host: "imap.163.com",
    imap_port: 993,
    imap_security: "tls",
    smtp_host: "smtp.163.com",
    smtp_port: 465,
    smtp_security: "tls",
  },
};

interface Props {
  onClose: () => void;
}

export default function AccountSetup({ onClose }: Props) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const emailInputRef = useRef<HTMLInputElement>(null);
  const realtimeMode = useUIStore((state) => state.realtimeMode);
  const syncPollInterval = realtimePreferenceToPollInterval(realtimeMode);

  const initialForm: AddAccountRequest = {
    email: "",
    display_name: "",
    provider: "imap",
    imap_host: "",
    imap_port: 993,
    imap_security: "tls",
    smtp_host: "",
    smtp_port: 587,
    smtp_security: "starttls",
    username: "",
    password: "",
  };
  const [form, setForm] = useState<AddAccountRequest>(initialForm);
  const initialFormRef = useRef(initialForm);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testLoading, setTestLoading] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [googleLoading, setGoogleLoading] = useState(false);
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleGoogleSignIn = async () => {
    setGoogleLoading(true);
    setError(null);
    try {
      const { sessionId, authUrl } = await startGoogleOAuth();
      const popup = window.open(authUrl, "google_oauth", "width=520,height=640,scrollbars=yes");

      const poll = async () => {
        try {
          const status = await pollGoogleOAuth(sessionId);
          if (status.status === "pending") {
            if (!popup || popup.closed) {
              setGoogleLoading(false);
              setError(t("accountSetup.oauthCancelled", "Sign in was cancelled"));
              return;
            }
            pollTimerRef.current = setTimeout(() => void poll(), 1500);
            return;
          }
          if (status.status === "complete") {
            popup?.close();
            await queryClient.invalidateQueries({ queryKey: accountsQueryKey });
            await queryClient.refetchQueries({ queryKey: accountsQueryKey });
            useToastStore.getState().addToast({
              message: t("accountSetup.accountAdded", "Account added successfully"),
              type: "success",
            });
            refreshFoldersAfterSyncStart(queryClient, status.account_id);
            onClose();
          } else {
            popup?.close();
            setError(status.message);
          }
        } catch {
          setError(t("accountSetup.oauthFailed", "Google sign in failed. Please try again."));
        } finally {
          setGoogleLoading(false);
        }
      };
      pollTimerRef.current = setTimeout(() => void poll(), 1500);
    } catch (err) {
      setError(extractErrorMessage(err));
      setGoogleLoading(false);
    }
  };

  useEffect(() => {
    return () => {
      if (pollTimerRef.current) clearTimeout(pollTimerRef.current);
    };
  }, []);

  const dialogRef = useRef<HTMLDivElement>(null);
  const formRef = useRef(form);
  formRef.current = form;

  const requestClose = async () => {
    const current = formRef.current;
    const initial = initialFormRef.current;
    const isDirty = (Object.keys(current) as Array<keyof AddAccountRequest>).some(
      (k) => current[k] !== initial[k],
    );
    if (!isDirty) {
      onClose();
      return;
    }
    const confirmed = await useConfirmStore.getState().confirm({
      title: i18n.t("accountSetup.discardTitle", "Discard form"),
      message: i18n.t("accountSetup.discardConfirm", "Discard this form?"),
      destructive: true,
    });
    if (confirmed) onClose();
  };
  const requestCloseRef = useRef(requestClose);
  requestCloseRef.current = requestClose;

  useEffect(() => {
    const previousFocus =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;

    emailInputRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        void requestCloseRef.current();
        return;
      }
      // Focus trap: keep Tab within the dialog
      if (event.key === "Tab" && dialogRef.current) {
        const focusable = dialogRef.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
        );
        if (focusable.length === 0) return;
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

  async function handleTestConnection() {
    setTestResult(null);
    setTestLoading(true);
    try {
      const report = await testImapConnection(
        form.imap_host,
        form.imap_port,
        form.imap_security,
        form.proxy_host,
        form.proxy_port,
        form.username || undefined,
        form.password || undefined,
      );
      setTestResult({ ok: true, message: report });
    } catch (err) {
      const msg = extractErrorMessage(err);
      setTestResult({ ok: false, message: msg });
    } finally {
      setTestLoading(false);
    }
  }

  function applyPreset(key: string) {
    const preset = PRESETS[key];
    if (!preset) return;
    setForm((prev) => ({ ...prev, ...preset }));
  }

  function handleChange(field: keyof AddAccountRequest, value: string | number | boolean) {
    setForm((prev) => {
      const updated = { ...prev, [field]: value };
      // Keep username in sync with email when username hasn't been manually changed
      if (field === "email" && prev.username === prev.email) {
        updated.username = value as string;
      }
      return updated;
    });
  }

  async function handleSubmit(e: React.SyntheticEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const account = await addAccount(form);
      // Invalidate accounts immediately so UI reflects the new account
      await queryClient.invalidateQueries({ queryKey: accountsQueryKey });
      onClose();
      useToastStore.getState().addToast({
        message: t("accountSetup.accountAdded", "Account added successfully"),
        type: "success",
      });
      // Start sync in background; poll folders until they appear
      startSync(account.id, syncPollInterval).catch((err) =>
        console.warn("Initial sync failed (will retry later):", err),
      );
      // Poll for folders a few times so sidebar updates without manual refresh
      refreshFoldersAfterSyncStart(queryClient, account.id);
    } catch (err) {
      setError(extractErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  const fieldStyle: React.CSSProperties = {
    display: "flex",
    flexDirection: "column",
    gap: "0",
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="account-setup-title"
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
        ref={dialogRef}
        style={{
          width: "min(480px, calc(100vw - 32px))",
          backgroundColor: "var(--color-bg)",
          borderRadius: "10px",
          boxShadow: "0 20px 60px rgba(0,0,0,0.3)",
          display: "flex",
          flexDirection: "column",
          maxHeight: "90vh",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "16px 20px",
            borderBottom: "1px solid var(--color-border)",
          }}
        >
          <h2
            id="account-setup-title"
            style={{
              margin: 0,
              fontSize: "15px",
              fontWeight: 600,
              color: "var(--color-text-primary)",
            }}
          >
            {t("accountSetup.title", "Add Email Account")}
          </h2>
          <button
            onClick={() => void requestClose()}
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

        {/* Scrollable body */}
        <div className="scroll-region account-setup-scroll" style={{ overflowY: "auto", padding: "20px" }}>
          {/* Google OAuth */}
          <div style={{ marginBottom: "20px" }}>
            <button
              type="button"
              onClick={() => void handleGoogleSignIn()}
              disabled={googleLoading}
              style={{
                width: "100%",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                gap: "10px",
                padding: "10px 16px",
                borderRadius: "8px",
                border: "1px solid var(--color-border)",
                backgroundColor: "var(--color-bg)",
                color: "var(--color-text-primary)",
                fontSize: "14px",
                fontWeight: 500,
                cursor: googleLoading ? "not-allowed" : "pointer",
                opacity: googleLoading ? 0.7 : 1,
              }}
            >
              {!googleLoading && (
                <svg width="18" height="18" viewBox="0 0 24 24">
                  <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
                  <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
                  <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l3.66-2.84z"/>
                  <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
                </svg>
              )}
              {googleLoading
                ? t("accountSetup.signingIn", "Signing in...")
                : t("accountSetup.signInGoogle", "Sign in with Google")}
            </button>

            <div style={{ display: "flex", alignItems: "center", gap: "10px", margin: "16px 0 0" }}>
              <div style={{ flex: 1, height: "1px", backgroundColor: "var(--color-border)" }} />
              <span style={{ fontSize: "12px", color: "var(--color-text-secondary)" }}>
                {t("accountSetup.orManual", "or add account manually")}
              </span>
              <div style={{ flex: 1, height: "1px", backgroundColor: "var(--color-border)" }} />
            </div>
          </div>

          {/* Quick setup presets */}
          <div style={{ marginBottom: "20px" }}>
            <span style={{ ...labelStyle, marginBottom: "8px" }}>{t("accountSetup.quickSetup", "Quick setup")}</span>
            <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
              {Object.keys(PRESETS).map((key) => (
                <button
                  key={key}
                  type="button"
                  onClick={() => applyPreset(key)}
                  style={{
                    padding: "5px 14px",
                    borderRadius: "20px",
                    border: "1px solid var(--color-border)",
                    backgroundColor: "transparent",
                    color: "var(--color-text-primary)",
                    fontSize: "12px",
                    cursor: "pointer",
                    textTransform: "capitalize",
                  }}
                >
                  {key === "163" ? "163" : key.charAt(0).toUpperCase() + key.slice(1)}
                </button>
              ))}
            </div>
          </div>

          <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
            {/* Email */}
            <div style={fieldStyle}>
              <label htmlFor="setup-email" style={labelStyle}>{t("accountSetup.emailAddress", "Email address")}</label>
                <input
                  ref={emailInputRef}
                  id="setup-email"
                  name="email"
                  autoComplete="email"
                style={inputStyle}
                type="email"
                required
                value={form.email}
                onChange={(e) => handleChange("email", e.target.value)}
                placeholder={t("accountSetup.emailPlaceholder", "you@example.com")}
              />
            </div>

            {/* Display name */}
            <div style={fieldStyle}>
              <label htmlFor="setup-display-name" style={labelStyle}>{t("accountSetup.displayName", "Display name")}</label>
              <input
                id="setup-display-name"
                name="display_name"
                autoComplete="name"
                style={inputStyle}
                type="text"
                required
                value={form.display_name}
                onChange={(e) => handleChange("display_name", e.target.value)}
                placeholder={t("accountSetup.namePlaceholder", "Your Name")}
              />
            </div>

            {/* IMAP */}
            <div style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: "12px" }}>
              <div style={fieldStyle}>
                <label htmlFor="setup-imap-host" style={labelStyle}>{t("accountSetup.imapHost", "IMAP host")}</label>
                <input
                  id="setup-imap-host"
                  name="imap_host"
                  style={inputStyle}
                  type="text"
                  required
                  value={form.imap_host}
                  onChange={(e) => handleChange("imap_host", e.target.value)}
                  placeholder="imap.example.com"
                />
              </div>
              <div style={fieldStyle}>
                <label htmlFor="setup-imap-port" style={labelStyle}>{t("accountSetup.imapPort", "IMAP port")}</label>
                <input
                  id="setup-imap-port"
                  name="imap_port"
                  style={{ ...inputStyle, width: "70px" }}
                  type="number"
                  required
                  value={form.imap_port}
                  onChange={(e) => handleChange("imap_port", parseInt(e.target.value, 10))}
                />
              </div>
              <div style={fieldStyle}>
                <label htmlFor="setup-imap-security" style={labelStyle}>{t("accountSetup.security", "Security")}</label>
                <select
                  id="setup-imap-security"
                  value={form.imap_security}
                  onChange={(e) => handleChange("imap_security", e.target.value)}
                  style={{ ...inputStyle, width: "110px" }}
                >
                  <option value="tls">{t("accountSetup.securityTls", "SSL/TLS")}</option>
                  <option value="starttls">{t("accountSetup.securityStarttls", "STARTTLS")}</option>
                </select>
              </div>
            </div>

            {/* SMTP */}
            <div style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: "12px" }}>
              <div style={fieldStyle}>
                <label htmlFor="setup-smtp-host" style={labelStyle}>{t("accountSetup.smtpHost", "SMTP host")}</label>
                <input
                  id="setup-smtp-host"
                  name="smtp_host"
                  style={inputStyle}
                  type="text"
                  required
                  value={form.smtp_host}
                  onChange={(e) => handleChange("smtp_host", e.target.value)}
                  placeholder="smtp.example.com"
                />
              </div>
              <div style={fieldStyle}>
                <label htmlFor="setup-smtp-port" style={labelStyle}>{t("accountSetup.smtpPort", "SMTP port")}</label>
                <input
                  id="setup-smtp-port"
                  name="smtp_port"
                  style={{ ...inputStyle, width: "70px" }}
                  type="number"
                  required
                  value={form.smtp_port}
                  onChange={(e) => handleChange("smtp_port", parseInt(e.target.value, 10))}
                />
              </div>
              <div style={fieldStyle}>
                <label htmlFor="setup-smtp-security" style={labelStyle}>{t("accountSetup.security", "Security")}</label>
                <select
                  id="setup-smtp-security"
                  value={form.smtp_security}
                  onChange={(e) => handleChange("smtp_security", e.target.value)}
                  style={{ ...inputStyle, width: "110px" }}
                >
                  <option value="tls">{t("accountSetup.securityTls", "SSL/TLS")}</option>
                  <option value="starttls">{t("accountSetup.securityStarttls", "STARTTLS")}</option>
                </select>
              </div>
            </div>

            {/* Username */}
            <div style={fieldStyle}>
              <label htmlFor="setup-username" style={labelStyle}>{t("accountSetup.username", "Username")}</label>
              <input
                id="setup-username"
                name="username"
                autoComplete="username"
                style={inputStyle}
                type="text"
                required
                value={form.username}
                onChange={(e) => handleChange("username", e.target.value)}
                placeholder={t("accountSetup.usernameHint", "Defaults to email address")}
              />
            </div>

            {/* Password */}
            <div style={fieldStyle}>
              <label htmlFor="setup-password" style={labelStyle}>{t("accountSetup.password", "Password / App password")}</label>
              <input
                id="setup-password"
                name="password"
                autoComplete="current-password"
                style={inputStyle}
                type="password"
                required
                value={form.password}
                onChange={(e) => handleChange("password", e.target.value)}
              />
            </div>

            {/* Sync Strategy */}
            <div style={{ ...fieldStyle, marginTop: "8px" }}>
              <label style={labelStyle}>{t("settings.syncStrategy", "Sync Strategy")}</label>
              <div style={{ display: "flex", flexDirection: "column", gap: "10px", marginTop: "10px" }}>
                <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
                  <input
                    type="radio"
                    name="sync_strategy"
                    value="recent"
                    checked={form.sync_strategy === 'recent' || !form.sync_strategy}
                    onChange={(e) => handleChange("sync_strategy", e.target.value)}
                  />
                  <span style={{ fontSize: "13px" }}>{t("settings.syncStrategyRecent", "Sync recent messages only (recommended)")}</span>
                </label>

                <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
                  <input
                    type="radio"
                    name="sync_strategy"
                    value="all"
                    checked={form.sync_strategy === 'all'}
                    onChange={(e) => handleChange("sync_strategy", e.target.value)}
                  />
                  <span style={{ fontSize: "13px" }}>{t("settings.syncStrategyAll", "Sync all messages")}</span>
                </label>

                <label style={{ display: "flex", alignItems: "center", gap: "8px", cursor: "pointer" }}>
                  <input
                    type="radio"
                    name="sync_strategy"
                    value="since_date"
                    checked={form.sync_strategy === 'since_date'}
                    onChange={(e) => handleChange("sync_strategy", e.target.value)}
                  />
                  <span style={{ fontSize: "13px" }}>{t("settings.syncStrategySinceDate", "Sync messages since a specific date")}</span>
                </label>

                {form.sync_strategy === 'since_date' && (
                  <div style={{ marginLeft: "24px" }}>
                    <input
                      type="date"
                      value={form.sync_since_date || ''}
                      onChange={(e) => handleChange("sync_since_date", e.target.value)}
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
              {(form.sync_strategy === 'all' || form.sync_strategy === 'since_date') && (
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

            {/* Test Connection */}
            {testResult && (
              <div
                role={testResult.ok ? "status" : "alert"}
                aria-live={testResult.ok ? "polite" : "assertive"}
                style={{
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

            {/* Error */}
            {error && (
              <div
                role="alert"
                aria-live="assertive"
                style={{
                  padding: "10px 12px",
                  borderRadius: "6px",
                  backgroundColor: "rgba(239,68,68,0.1)",
                  border: "1px solid rgba(239,68,68,0.3)",
                  color: "#ef4444",
                  fontSize: "13px",
                }}
              >
                {error}
              </div>
            )}

            {/* Buttons */}
            <div style={{ display: "flex", gap: "10px", marginTop: "4px" }}>
              <button
                type="button"
                disabled={testLoading || !form.imap_host}
                onClick={handleTestConnection}
                style={{
                  padding: "9px 16px",
                  borderRadius: "6px",
                  border: "1px solid var(--color-border)",
                  backgroundColor: "transparent",
                  color: "var(--color-text-primary)",
                  fontSize: "13px",
                  fontWeight: 500,
                  cursor: testLoading || !form.imap_host ? "not-allowed" : "pointer",
                  opacity: testLoading || !form.imap_host ? 0.6 : 1,
                }}
              >
                {testLoading ? t("accountSetup.testing", "Testing...") : t("accountSetup.testConnection", "Test Connection")}
              </button>
              <button
                type="submit"
                disabled={loading}
                style={{
                  flex: 1,
                  padding: "9px 16px",
                  borderRadius: "6px",
                  border: "none",
                  backgroundColor: "var(--color-accent)",
                  color: "#fff",
                  fontSize: "13px",
                  fontWeight: 600,
                  cursor: loading ? "not-allowed" : "pointer",
                  opacity: loading ? 0.7 : 1,
                }}
              >
                {loading ? t("accountSetup.adding", "Adding account…") : t("accountSetup.submit", "Add Account & Sync")}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}
