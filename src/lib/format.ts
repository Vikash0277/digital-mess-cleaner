// Formatting helpers. Mirrors src-tauri/src/format.rs so the UI and the
// backend agree on sizes down to the last decimal.

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

export function formatBytes(value: number): string {
  const safe = Math.max(0, Number.isFinite(value) ? value : 0);
  let size = safe;
  let index = 0;
  while (size >= 1024 && index < UNITS.length - 1) {
    size /= 1024;
    index += 1;
  }
  const unit = UNITS[index] ?? "B";
  if (index === 0 || size >= 100) return `${size.toFixed(0)} ${unit}`;
  return `${size.toFixed(1)} ${unit}`;
}

export function formatCount(value: number): string {
  return new Intl.NumberFormat(undefined).format(Math.max(0, value));
}

export function formatDate(seconds: number | null | undefined): string {
  if (!seconds) return "—";
  return new Date(seconds * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function formatDateTime(seconds: number | null | undefined): string {
  if (!seconds) return "—";
  return new Date(seconds * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function relativeTime(seconds: number | null | undefined): string {
  if (!seconds) return "Never";
  const diff = Date.now() / 1000 - seconds;
  if (diff < 60) return "Just now";
  if (diff < 3600) return `${Math.floor(diff / 60)} min ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} h ago`;
  const days = Math.floor(diff / 86400);
  if (days === 1) return "Yesterday";
  if (days < 30) return `${days} days ago`;
  return formatDate(seconds);
}

export function categoryLabel(category: string): string {
  switch (category) {
    case "documents":
      return "Documents";
    case "images":
      return "Images";
    case "video":
      return "Video";
    case "audio":
      return "Audio";
    case "archives":
      return "Archives";
    case "installers":
      return "Installers";
    default:
      return "Other";
  }
}

export function pluralize(count: number, singular: string, plural?: string): string {
  return count === 1 ? singular : (plural ?? `${singular}s`);
}
