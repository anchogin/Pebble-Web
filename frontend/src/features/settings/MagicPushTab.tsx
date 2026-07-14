import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getMagicPushConfig,
  saveMagicPushConfig,
  testMagicPushConfig,
  type SaveMagicPushConfigRequest,
} from "@/lib/api";
import { extractErrorMessage } from "@/lib/extractErrorMessage";
import { useToastStore } from "@/stores/toast.store";

const labelStyle: React.CSSProperties = {
  display: "block",
  fontSize: "12px",
  fontWeight: 500,
  color: "var(--color-text-secondary)",
  marginBottom: "4px",
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  fontSize: "13px",
  border: "1px solid var(--color-border)",
  borderRadius: "6px",
  background: "var(--color-bg-secondary)",
  color: "var(--color-text-primary)",
  boxSizing: "border-box",
};

const fieldGroupStyle: React.CSSProperties = {
  marginBottom: "14px",
};

const buttonStyle: React.CSSProperties = {
  padding: "8px 18px",
  fontSize: "13px",
  fontWeight: 500,
  border: "none",
  borderRadius: "6px",
  cursor: "pointer",
};

export default function MagicPushTab() {
  const { t } = useTranslation();
  const addToast = useToastStore((s) => s.addToast);
  const [enabled, setEnabled] = useState(false);
  const [baseUrl, setBaseUrl] = useState("");
  const [token, setToken] = useState("");
  const [hasToken, setHasToken] = useState(false);
  const [clearToken, setClearToken] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [statusMsg, setStatusMsg] = useState("");
  const [statusType, setStatusType] = useState<"success" | "error" | "">("");

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getMagicPushConfig()
      .then((config) => {
        if (cancelled) return;
        setEnabled(config.enabled);
        setBaseUrl(config.baseUrl);
        setHasToken(config.hasToken);
        setToken("");
        setClearToken(false);
      })
      .catch((err) => {
        if (cancelled) return;
        const message = extractErrorMessage(err);
        setStatusMsg(t("magicPush.loadFailed", { error: message }));
        setStatusType("error");
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [t]);

  function buildRequest(): SaveMagicPushConfigRequest {
    return {
      enabled,
      baseUrl: baseUrl.trim(),
      token: token.trim() || null,
      clearToken,
    };
  }

  async function handleSave() {
    setSaving(true);
    setStatusMsg("");
    try {
      await saveMagicPushConfig(buildRequest());
      setHasToken(hasToken || token.trim().length > 0 ? !clearToken || token.trim().length > 0 : false);
      setToken("");
      setClearToken(false);
      setStatusMsg(t("magicPush.configSaved"));
      setStatusType("success");
      addToast({ message: t("magicPush.configSaved"), type: "success" });
    } catch (err) {
      const message = extractErrorMessage(err);
      setStatusMsg(t("magicPush.saveFailed", { error: message }));
      setStatusType("error");
      addToast({ message: t("magicPush.saveFailed", { error: message }), type: "error" });
    } finally {
      setSaving(false);
    }
  }

  async function handleTest() {
    setTesting(true);
    setStatusMsg("");
    try {
      await testMagicPushConfig(buildRequest());
      setStatusMsg(t("magicPush.testSent"));
      setStatusType("success");
      addToast({ message: t("magicPush.testSent"), type: "success" });
    } catch (err) {
      const message = extractErrorMessage(err);
      setStatusMsg(t("magicPush.testFailed", { error: message }));
      setStatusType("error");
      addToast({ message: t("magicPush.testFailed", { error: message }), type: "error" });
    } finally {
      setTesting(false);
    }
  }

  return (
    <div>
      <h2 style={{ fontSize: "18px", fontWeight: 600, color: "var(--color-text-primary)", marginTop: 0, marginBottom: "8px" }}>
        {t("magicPush.title")}
      </h2>
      <p style={{ fontSize: "12px", color: "var(--color-text-secondary)", marginTop: 0, marginBottom: "18px", lineHeight: 1.6 }}>
        {t("magicPush.description")}
      </p>

      <div style={{ ...fieldGroupStyle, display: "flex", alignItems: "center", gap: "8px" }}>
        <input
          id="magicpush-enabled"
          type="checkbox"
          checked={enabled}
          disabled={loading || saving || testing}
          onChange={(event) => setEnabled(event.target.checked)}
        />
        <label htmlFor="magicpush-enabled" style={{ fontSize: "13px", color: "var(--color-text-primary)", cursor: "pointer" }}>
          {t("magicPush.enable")}
        </label>
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="magicpush-base-url" style={labelStyle}>{t("magicPush.baseUrl")}</label>
        <input
          id="magicpush-base-url"
          type="url"
          value={baseUrl}
          disabled={loading || saving || testing}
          onChange={(event) => setBaseUrl(event.target.value)}
          placeholder="https://push.example.com"
          autoComplete="off"
          style={inputStyle}
        />
      </div>

      <div style={fieldGroupStyle}>
        <label htmlFor="magicpush-token" style={labelStyle}>{t("magicPush.token")}</label>
        <input
          id="magicpush-token"
          type="password"
          value={token}
          disabled={loading || saving || testing || clearToken}
          onChange={(event) => setToken(event.target.value)}
          placeholder={hasToken ? t("magicPush.tokenSavedPlaceholder") : t("magicPush.tokenPlaceholder")}
          autoComplete="current-password"
          style={inputStyle}
        />
        {hasToken && (
          <p style={{ margin: "6px 0 0", fontSize: "12px", color: "var(--color-text-secondary)" }}>
            {t("magicPush.tokenSavedHint")}
          </p>
        )}
      </div>

      {hasToken && (
        <div style={{ ...fieldGroupStyle, display: "flex", alignItems: "center", gap: "8px" }}>
          <input
            id="magicpush-clear-token"
            type="checkbox"
            checked={clearToken}
            disabled={loading || saving || testing}
            onChange={(event) => setClearToken(event.target.checked)}
          />
          <label htmlFor="magicpush-clear-token" style={{ fontSize: "13px", color: "var(--color-text-primary)", cursor: "pointer" }}>
            {t("magicPush.clearToken")}
          </label>
        </div>
      )}

      <div style={{ display: "flex", gap: "10px", marginTop: "20px", flexWrap: "wrap" }}>
        <button
          type="button"
          style={{ ...buttonStyle, background: "var(--color-accent)", color: "#fff", opacity: saving || loading ? 0.6 : 1 }}
          onClick={handleSave}
          disabled={saving || loading}
        >
          {saving ? t("common.saving") : t("common.save")}
        </button>
        <button
          type="button"
          style={{ ...buttonStyle, background: "var(--color-bg-hover)", color: "var(--color-text-primary)", opacity: testing || loading ? 0.6 : 1 }}
          onClick={handleTest}
          disabled={testing || loading}
        >
          {testing ? t("common.testing") : t("magicPush.sendTest")}
        </button>
      </div>

      {statusMsg && (
        <div
          role={statusType === "error" ? "alert" : "status"}
          aria-live="polite"
          style={{
            marginTop: "14px",
            padding: "10px 14px",
            borderRadius: "6px",
            fontSize: "13px",
            background: statusType === "success" ? "var(--color-bg-hover)" : "rgba(220, 53, 69, 0.1)",
            color: statusType === "success" ? "var(--color-text-primary)" : "#dc3545",
            border: `1px solid ${statusType === "success" ? "var(--color-border)" : "rgba(220, 53, 69, 0.3)"}`,
          }}
        >
          {statusMsg}
        </div>
      )}
    </div>
  );
}
