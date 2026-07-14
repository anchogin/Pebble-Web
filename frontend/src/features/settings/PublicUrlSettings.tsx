import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getGeneralSettings, saveGeneralSettings } from "@/lib/api";
import { extractErrorMessage } from "@/lib/extractErrorMessage";
import { useToastStore } from "@/stores/toast.store";

const fieldGroupStyle: React.CSSProperties = {
  marginBottom: "14px",
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

export default function PublicUrlSettings() {
  const { t } = useTranslation();
  const addToast = useToastStore((s) => s.addToast);
  const [publicUrl, setPublicUrl] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getGeneralSettings()
      .then((settings) => {
        if (!cancelled) setPublicUrl(settings.publicUrl);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        const message = extractErrorMessage(error);
        addToast({ message: t("settings.publicUrlLoadFailed", { error: message }), type: "error" });
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [addToast, t]);

  const handleSave = useCallback(async () => {
    const trimmed = publicUrl.trim();
    setSaving(true);
    try {
      await saveGeneralSettings({ publicUrl: trimmed });
      setPublicUrl(trimmed);
      addToast({ message: t("settings.publicUrlSaved"), type: "success" });
    } catch (error: unknown) {
      const message = extractErrorMessage(error);
      addToast({ message: t("settings.publicUrlSaveFailed", { error: message }), type: "error" });
    } finally {
      setSaving(false);
    }
  }, [addToast, publicUrl, t]);

  return (
    <section style={{ marginBottom: "32px" }}>
      <h3 style={{ fontSize: "14px", fontWeight: 600, marginBottom: "8px" }}>
        {t("settings.publicUrl")}
      </h3>
      <p style={{ fontSize: "12px", color: "var(--color-text-secondary)", marginBottom: "12px", marginTop: 0 }}>
        {t("settings.publicUrlDesc")}
      </p>
      <div style={fieldGroupStyle}>
        <input
          id="general-public-url"
          type="url"
          value={publicUrl}
          disabled={loading || saving}
          onChange={(event) => setPublicUrl(event.target.value)}
          placeholder="https://mail.example.com"
          autoComplete="off"
          style={inputStyle}
        />
      </div>
      <button
        type="button"
        onClick={handleSave}
        disabled={loading || saving}
        style={{
          padding: "8px 12px",
          borderRadius: "6px",
          border: "1px solid var(--color-border)",
          backgroundColor: "var(--color-bg)",
          color: "var(--color-text-primary)",
          cursor: loading || saving ? "not-allowed" : "pointer",
          fontSize: "13px",
        }}
      >
        {saving ? t("common.saving") : t("common.save")}
      </button>
    </section>
  );
}
