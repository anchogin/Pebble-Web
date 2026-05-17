import iconUrl from "@/assets/app-icon.png";

export default function TitleBar() {
  return (
    <div
      className="flex items-center h-9 select-none px-3 gap-2"
      style={{ backgroundColor: "var(--color-titlebar-bg)" }}
    >
      <img
        src={iconUrl}
        alt=""
        aria-hidden="true"
        draggable={false}
        className="h-5 w-5 shrink-0 bg-transparent object-contain"
      />
      <span
        className="text-sm font-semibold"
        style={{ color: "var(--color-text-primary)" }}
      >
        Pebble
      </span>
    </div>
  );
}
