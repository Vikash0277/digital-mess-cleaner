import { FolderTree, TrendingUp } from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { FileTable } from "../components/FileTable";
import { ErrorNotice, LoadingNotice } from "../components/Notice";
import { ProgressBar } from "../components/ProgressBar";
import { StatCard } from "../components/StatCard";
import { api } from "../lib/api";
import {
  categoryLabel,
  formatBytes,
  formatCount,
  formatDate,
  pluralize,
  relativeTime,
} from "../lib/format";
import { useAsync } from "../lib/hooks";

const loadInsights = async () => {
  const [dashboard, files, folders, trends, growth] = await Promise.all([
    api.dashboard(),
    api.largestFiles(15),
    api.largestFolders(10),
    api.trends(90),
    api.growth(30),
  ]);
  return { dashboard, files, folders, trends, growth };
};

function signedBytes(value: number): string {
  if (value === 0) return "No change";
  return `${value > 0 ? "+" : "−"}${formatBytes(Math.abs(value))}`;
}

export function StorageInsightsPage() {
  const insights = useAsync(loadInsights);
  const data = insights.data;

  const trendPoints = data ? [...data.trends].sort((a, b) => a.takenAt - b.takenAt).slice(-10) : [];
  const topFolderBytes = data?.folders[0]?.bytes ?? 0;
  const growthDelta =
    data?.growth.categories.reduce((sum, entry) => sum + entry.deltaBytes, 0) ?? 0;

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">Storage Insights</h1>
          <p className="page-subtitle">What is using space, and where it is heading</p>
        </div>
      </div>

      {insights.loading && !data && (
        <LoadingNotice label="Reading storage snapshots from your local index…" />
      )}

      {insights.error && (
        <ErrorNotice
          message={insights.error}
          onRetry={insights.reload}
          desktopOnly={insights.errorCode === "not_desktop"}
        />
      )}

      {data && data.dashboard.fileCount === 0 && (
        <EmptyState
          title="No storage data yet"
          description="Add a folder on the Dashboard and run a scan. Insights appear once files are indexed."
          icon="trend-up"
        />
      )}

      {data && data.dashboard.fileCount > 0 && (
        <>
          <div className="stats-grid">
            <StatCard
              title="Library size"
              value={formatBytes(data.dashboard.totalBytes)}
              subtitleOf={`${formatCount(data.dashboard.fileCount)} indexed files`}
            />
            <StatCard
              title="Change (30 days)"
              value={signedBytes(growthDelta)}
              subtitleOf="Across all categories"
            />
            <StatCard
              title="Biggest folder"
              value={formatBytes(topFolderBytes)}
              subtitleOf={data.folders[0]?.path ?? "No folders yet"}
            />
            <StatCard
              title="Clutter score"
              value={`${data.dashboard.clutter.score}/100`}
              subtitleOf={data.dashboard.clutter.band}
            />
          </div>

          {data.dashboard.prediction && (
            <section className="card prediction-card" aria-labelledby="insights-forecast-title">
              <h2 id="insights-forecast-title" className="section-title">
                <TrendingUp size={16} aria-hidden="true" /> Space forecast
              </h2>
              <p>{data.dashboard.prediction.message}</p>
              <p className="prediction-meta">
                Based on {data.dashboard.prediction.basedOnDays} days of snapshots ·
                confidence {Math.round(data.dashboard.prediction.confidence * 100)}%
              </p>
              {data.dashboard.prediction.growthPerDay > 0 && (
                <p className="prediction-meta">
                  Growth: {formatBytes(data.dashboard.prediction.growthPerDay)} per day
                </p>
              )}
            </section>
          )}

          <section className="card category-summary" aria-labelledby="insights-categories-title">
            <div className="section-head">
              <h2 id="insights-categories-title" className="section-title">
                By category
              </h2>
              <span className="muted">Sorted by size</span>
            </div>
            <ul className="category-summary-list">
              {[...data.dashboard.byCategory]
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

          <section className="card" aria-labelledby="insights-folders-title">
            <div className="section-head">
              <h2 id="insights-folders-title" className="section-title">
                <FolderTree size={16} aria-hidden="true" /> Largest folders
              </h2>
              <span className="muted">Share of the biggest folder</span>
            </div>

            {data.folders.length === 0 ? (
              <p className="muted">No folder totals yet. Run a scan to build them.</p>
            ) : (
              <ul className="folder-size-list">
                {data.folders.map((folder) => (
                  <li key={folder.path} className="folder-size-item">
                    <div className="folder-size-head">
                      <span className="folder-size-path" title={folder.path}>
                        {folder.path}
                      </span>
                      <span className="muted">
                        {formatBytes(folder.bytes)} · {formatCount(folder.count)}{" "}
                        {pluralize(folder.count, "file")}
                      </span>
                    </div>
                    <ProgressBar
                      progress={topFolderBytes > 0 ? (folder.bytes / topFolderBytes) * 100 : 0}
                      label={`${folder.path} size share`}
                    />
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="card" aria-labelledby="insights-growth-title">
            <h2 id="insights-growth-title" className="section-title">
              Growth by category
            </h2>
            <p className="muted">
              Compared with the snapshot {relativeTime(data.growth.since)}.{" "}
              {data.growth.until ? `Latest snapshot ${relativeTime(data.growth.until)}.` : ""}
            </p>

            {data.growth.categories.length === 0 ? (
              <p className="muted">
                Not enough snapshots yet. Insights compare at least two scans over time.
              </p>
            ) : (
              <ul className="category-summary-list">
                {[...data.growth.categories]
                  .sort((a, b) => Math.abs(b.deltaBytes) - Math.abs(a.deltaBytes))
                  .map((entry) => (
                    <li key={entry.category} className="category-summary-item">
                      <span>{categoryLabel(entry.category)}</span>
                      <span>{formatBytes(entry.toBytes)}</span>
                      <span
                        className={
                          entry.deltaBytes > 0
                            ? "delta-up"
                            : entry.deltaBytes < 0
                              ? "delta-down"
                              : undefined
                        }
                      >
                        {signedBytes(entry.deltaBytes)}
                      </span>
                    </li>
                  ))}
              </ul>
            )}

            {data.growth.notes.length > 0 && (
              <ul className="notes-list">
                {data.growth.notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            )}
          </section>

          <section className="card" aria-labelledby="insights-trend-title">
            <div className="section-head">
              <h2 id="insights-trend-title" className="section-title">
                Recent snapshots
              </h2>
              <span className="muted">Last {trendPoints.length}</span>
            </div>

            {trendPoints.length === 0 ? (
              <p className="muted">Snapshots are recorded after each scan.</p>
            ) : (
              <ul className="snapshot-list">
                {[...trendPoints].reverse().map((point, index, all) => {
                  const previous = all[index + 1];
                  const delta = previous ? point.totalBytes - previous.totalBytes : 0;
                  return (
                    <li key={point.takenAt} className="snapshot-item">
                      <span>{formatDate(point.takenAt)}</span>
                      <span>{formatBytes(point.totalBytes)}</span>
                      <span className="muted">{formatCount(point.fileCount)} files</span>
                      <span
                        className={
                          delta > 0 ? "delta-up" : delta < 0 ? "delta-down" : "muted"
                        }
                      >
                        {previous ? signedBytes(delta) : "First snapshot"}
                      </span>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>

          <section className="card" aria-labelledby="insights-files-title">
            <div className="section-head">
              <h2 id="insights-files-title" className="section-title">
                Largest files
              </h2>
              <span className="muted">Review before removing anything</span>
            </div>
            <div className="media-table">
              <FileTable files={data.files} caption="Largest files in your library" />
            </div>
          </section>

          {data.dashboard.clutter.factors.length > 0 && (
            <section className="card" aria-label="Clutter score evidence">
              <h2 className="section-title">Clutter score evidence</h2>
              <ul className="snapshot-list">
                {data.dashboard.clutter.factors.map((factor) => (
                  <li key={factor.label} className="snapshot-item">
                    <span>{factor.label}</span>
                    <span>{factor.value}</span>
                    <span className="muted">weight {factor.weight.toFixed(1)}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </>
      )}
    </div>
  );
}
