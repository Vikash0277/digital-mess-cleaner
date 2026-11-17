import { FolderPlus, Trash2 } from "lucide-react";
import { useCallback, useState } from "react";
import { EmptyState } from "../components/EmptyState";
import { ErrorNotice, LoadingNotice, SuccessNotice } from "../components/Notice";
import { ScanControls } from "../components/ScanControls";
import { StatCard } from "../components/StatCard";
import { StorageBar } from "../components/StorageBar";
import { api, errorMessage, pickFolder } from "../lib/api";
import {
  categoryLabel,
  formatBytes,
  formatCount,
  formatDateTime,
  pluralize,
  relativeTime,
} from "../lib/format";
import { bumpData, useAsync } from "../lib/hooks";
import { useReview } from "../lib/review";
import type { RootInfo } from "../lib/types";

const loadDashboard = () => api.dashboard();

export function DashboardPage() {
  const dashboard = useAsync(loadDashboard);
  const review = useReview();
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const addFolder = useCallback(async () => {
    setActionError(null);
    setNotice(null);
    setBusy(true);
    try {
      const path = await pickFolder();
      if (!path) return;
      await api.rootsAdd(path);
      bumpData();
      setNotice(`Added ${path}. Run a scan to index it.`);
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }, []);

  const removeRoot = useCallback(
    async (root: RootInfo) => {
      const confirmed = await review({
        title: `Remove ${root.label} from the library?`,
        message:
          "This only removes the folder from Digital Mess Cleaner's index. No files on disk are moved or deleted.",
        details: [
          { label: "Folder", value: root.path },
          { label: "Indexed files", value: formatCount(root.fileCount) },
          { label: "Indexed size", value: formatBytes(root.totalBytes) },
        ],
        note: "You can add it again at any time.",
        primaryText: "Remove from library",
        tone: "caution",
      });
      if (!confirmed) return;

      setActionError(null);
      setBusy(true);
      try {
        await api.rootsRemove(root.id);
        bumpData();
        setNotice(`${root.label} was removed from the library.`);
      } catch (error) {
        setActionError(errorMessage(error));
      } finally {
        setBusy(false);
      }
    },
    [review],
  );

  const data = dashboard.data;
  const hasRoots = (data?.roots.length ?? 0) > 0;

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">Dashboard</h1>
          <p className="page-subtitle">Overview of your digital storage</p>
        </div>
        <div className="page-heading-actions">
          <button
            type="button"
            className="btn"
            onClick={() => void addFolder()}
            disabled={busy}
          >
            <FolderPlus size={14} aria-hidden="true" />
            Add folder
          </button>
        </div>
      </div>

      {dashboard.error && (
        <ErrorNotice
          message={dashboard.error}
          onRetry={dashboard.reload}
          desktopOnly={dashboard.errorCode === "not_desktop"}
        />
      )}

      {actionError && <ErrorNotice message={actionError} />}
      {notice && <SuccessNotice message={notice} />}

      {dashboard.loading && !data && <LoadingNotice label="Reading your local index…" />}

      {!dashboard.loading && !data && !dashboard.error && (
        <EmptyState
          title="Nothing indexed yet"
          description="Add a folder or drive to begin. Digital Mess Cleaner only reads folders you choose."
          icon="hard-drive"
          buttonText="Add storage location"
          onAction={() => void addFolder()}
          actionDisabled={busy}
        />
      )}

      {data && (
        <>
          <ScanControls hasRoots={hasRoots} onNeedFolder={() => void addFolder()} />

          <StorageBar
            byCategory={data.byCategory}
            totalBytes={data.totalBytes}
            roots={data.roots}
            disks={data.disks}
          />

          <div className="stats-grid">
            <StatCard
              title="Files indexed"
              value={formatCount(data.fileCount)}
              subtitleOf={
                data.lastScanAt ? `Last scan ${relativeTime(data.lastScanAt)}` : "Not scanned yet"
              }
            />
            <StatCard
              title="Library size"
              value={formatBytes(data.totalBytes)}
              subtitleOf={`${data.byCategory.length} ${pluralize(data.byCategory.length, "category", "categories")}`}
            />
            <StatCard
              title="Exact duplicate groups"
              value={formatCount(data.duplicateGroups)}
              subtitleOf={`${formatBytes(data.duplicateBytes)} could be reclaimed`}
            />
            <StatCard
              title="Quarantine"
              value={`${formatCount(data.quarantineCount)} ${pluralize(data.quarantineCount, "file")}`}
              subtitleOf={`${formatBytes(data.quarantineBytes)} held safely`}
            />
            <StatCard
              title="Not opened in 180 days"
              value={formatCount(data.forgottenCount)}
              subtitleOf={`${formatBytes(data.forgottenBytes)} in old files`}
            />
            <StatCard
              title="Empty folders"
              value={formatCount(data.emptyFolderCount)}
              subtitleOf="Review before removing"
            />
          </div>

          {data.roots.length === 0 ? (
            <EmptyState
              title="No folders added"
              description="Add a folder or drive to begin scanning. Nothing is scanned until you say so."
              icon="hard-drive"
              buttonText="Add storage location"
              onAction={() => void addFolder()}
              actionDisabled={busy}
            />
          ) : (
            <section className="card locations-card" aria-labelledby="locations-title">
              <div className="section-head">
                <h2 id="locations-title" className="section-title">
                  Locations
                </h2>
                <button
                  type="button"
                  className="btn"
                  onClick={() => void addFolder()}
                  disabled={busy}
                >
                  <FolderPlus size={14} aria-hidden="true" />
                  Add folder
                </button>
              </div>

              <ul className="location-list">
                {data.roots.map((root) => (
                  <li key={root.id} className="location-item">
                    <div className="location-info">
                      <span className="location-label">{root.label}</span>
                      <span className="location-path" title={root.path}>
                        {root.path}
                      </span>
                    </div>
                    <div className="location-meta">
                      <span>{formatCount(root.fileCount)} files</span>
                      <span>{formatBytes(root.totalBytes)}</span>
                      <span>
                        {root.lastScanAt
                          ? `Scanned ${relativeTime(root.lastScanAt)}`
                          : "Not scanned yet"}
                      </span>
                    </div>
                    <button
                      type="button"
                      className="btn"
                      onClick={() => void removeRoot(root)}
                      aria-label={`Remove ${root.label} from the library`}
                      title="Remove from the library without touching files"
                      disabled={busy}
                    >
                      <Trash2 size={14} aria-hidden="true" />
                      Remove
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {data.byCategory.length > 0 && (
            <section className="card category-summary" aria-labelledby="categories-title">
              <h2 id="categories-title" className="section-title">
                What takes up space
              </h2>
              <ul className="category-summary-list">
                {[...data.byCategory]
                  .sort((a, b) => b.bytes - a.bytes)
                  .map((entry) => (
                    <li key={entry.category} className="category-summary-item">
                      <span>{categoryLabel(entry.category)}</span>
                      <span>{formatBytes(entry.bytes)}</span>
                      <span>{formatCount(entry.count)} files</span>
                    </li>
                  ))}
              </ul>
            </section>
          )}

          {data.prediction && (
            <section className="card prediction-card" aria-labelledby="prediction-title">
              <h2 id="prediction-title" className="section-title">
                Space forecast
              </h2>
              <p>{data.prediction.message}</p>
              <p className="prediction-meta">
                Based on {data.prediction.basedOnDays} days of snapshots · confidence{" "}
                {Math.round(data.prediction.confidence * 100)}%
              </p>
            </section>
          )}

          <section className="card activity-card" aria-labelledby="activity-title">
            <h2 id="activity-title" className="section-title">
              Recent activity
            </h2>
            {data.recentActivity.length === 0 ? (
              <p className="muted">No file operations yet. Everything you do will be listed here.</p>
            ) : (
              <ul className="activity-list">
                {data.recentActivity.map((entry) => (
                  <li key={entry.id} className="activity-item">
                    <span className="activity-summary">{entry.summary}</span>
                    <span className="activity-meta">
                      {formatDateTime(entry.createdAt)} ·{" "}
                      {entry.reversible ? "Reversible" : "Not reversible"}
                      {entry.undoneAt ? " · Undone" : ""}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </>
      )}
    </div>
  );
}
