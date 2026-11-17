import { formatBytes } from "../lib/format";
import type { CategoryStat, DiskInfo, RootInfo } from "../lib/types";

const CATEGORIES = [
  "documents",
  "images",
  "video",
  "audio",
  "archives",
  "installers",
  "other",
] as const;

const LABELS: Record<string, string> = {
  documents: "Documents",
  images: "Images",
  video: "Video",
  audio: "Audio",
  archives: "Archives",
  installers: "Installers",
  other: "Other",
};

function diskForRoots(roots: RootInfo[], disks: DiskInfo[]): DiskInfo | null {
  const usable = disks.filter((disk) => disk.total > 0);
  if (usable.length === 0) return null;

  const matched = usable.filter((disk) =>
    roots.some((root) => root.path.toLowerCase().startsWith(disk.mount.toLowerCase())),
  );
  const pool = matched.length > 0 ? matched : usable;

  return pool.reduce((best, disk) => (disk.total > best.total ? disk : best), pool[0]!);
}

export interface StorageBarProps {
  byCategory: CategoryStat[];
  totalBytes: number;
  roots?: RootInfo[];
  disks?: DiskInfo[];
}

export function StorageBar({ byCategory, totalBytes, roots = [], disks = [] }: StorageBarProps) {
  const counts = new Map(byCategory.map((entry) => [entry.category, entry]));
  const segments = CATEGORIES.map((category) => ({
    category,
    bytes: counts.get(category)?.bytes ?? 0,
    count: counts.get(category)?.count ?? 0,
  })).filter((segment) => segment.bytes > 0);

  const disk = diskForRoots(roots, disks);
  const capacity = disk?.total ?? 0;
  const free = disk && capacity > totalBytes ? Math.max(0, disk.available) : 0;
  const usedShare = capacity > 0 ? Math.min(100, Math.round((totalBytes / capacity) * 100)) : 0;

  return (
    <section className="card storage-card" aria-labelledby="storage-overview-title">
      <div className="storage-card-header">
        <h2 id="storage-overview-title" className="storage-card-title">
          Storage overview
        </h2>
        <span className="storage-card-meta">
          {formatBytes(totalBytes)} in your library
          {disk ? ` · ${formatBytes(disk.available)} free of ${formatBytes(capacity)} on ${disk.name}` : ""}
        </span>
      </div>

      {segments.length === 0 ? (
        <p className="storage-empty">
          Nothing indexed yet. Add a folder and run a scan to fill this in.
        </p>
      ) : (
        <>
          <div
            className="storage-bar"
            role="img"
            aria-label={
              disk
                ? `Library uses ${usedShare}% of ${disk.name}. ${formatBytes(totalBytes)} indexed, ${formatBytes(disk.available)} free.`
                : `Library total ${formatBytes(totalBytes)}.`
            }
          >
            {segments.map((segment) => (
              <span
                key={segment.category}
                data-cat={segment.category}
                style={{ flex: segment.bytes }}
                title={`${LABELS[segment.category]}: ${formatBytes(segment.bytes)}`}
              />
            ))}
            {free > 0 && (
              <span
                className="storage-free"
                style={{ flex: free }}
                title={`Free: ${formatBytes(free)}`}
              />
            )}
          </div>

          <ul className="storage-legend">
            {segments.map((segment) => (
              <li key={segment.category}>
                <span
                  className="storage-legend-swatch"
                  data-cat={segment.category}
                  aria-hidden="true"
                />
                {LABELS[segment.category]} · {formatBytes(segment.bytes)}
              </li>
            ))}
            {free > 0 && (
              <li>
                <span className="storage-legend-swatch storage-legend-free" aria-hidden="true" />
                Free · {formatBytes(free)}
              </li>
            )}
          </ul>
        </>
      )}
    </section>
  );
}
