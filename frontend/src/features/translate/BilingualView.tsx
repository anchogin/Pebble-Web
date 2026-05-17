interface Props {
  segments: { source: string; target: string }[];
}

export default function BilingualView({ segments }: Props) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1px" }}>
      {segments.map((seg, i) => (
        <div
          key={i}
          style={{
            display: "grid",
            gridTemplateColumns: "1fr 1fr",
            gap: "16px",
            padding: "8px 0",
            borderBottom: i < segments.length - 1 ? "1px solid var(--color-border)" : "none",
          }}
        >
          <div style={{ fontSize: "13px", lineHeight: "1.6", color: "var(--color-text-secondary)" }}>
            {seg.source}
          </div>
          <div style={{ fontSize: "13px", lineHeight: "1.6", color: "var(--color-text-primary)" }}>
            {seg.target}
          </div>
        </div>
      ))}
    </div>
  );
}
