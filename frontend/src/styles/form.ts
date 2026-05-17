import type { CSSProperties } from "react";

export const inputStyle: CSSProperties = {
  width: "100%",
  padding: "7px 10px",
  borderRadius: "6px",
  border: "1px solid var(--color-border)",
  backgroundColor: "var(--color-bg)",
  color: "var(--color-text-primary)",
  fontSize: "13px",
  boxSizing: "border-box",
};

export const labelStyle: CSSProperties = {
  display: "block",
  fontSize: "12px",
  color: "var(--color-text-secondary)",
  marginBottom: "4px",
};

export const fieldGroupStyle: CSSProperties = {
  marginBottom: "14px",
};
