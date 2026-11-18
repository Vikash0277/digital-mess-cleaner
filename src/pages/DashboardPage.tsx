import { FolderPlus, Search, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { FileTable } from "../components/FileTable";
import { ErrorNotice, LoadingNotice, SuccessNotice } from "../components/Notice";
import { ScanControls } from "../components/ScanControls";
import { StatCard } from "../components/StatCard";
import { StorageBar } from "../components/StorageBar";
import { api, errorMessage, pickFolder } from "../lib/api";
import {
  categoryLabel,
  formatBytes,
  formatCount,
  formatDate,
  formatDateTime,
  pluralize,
  relativeTime,
} from "../lib/format";
import { bumpData, useAsync } from "../lib/hooks";
import { useReview } from "../lib/review";
import type { CategoryDetails, InstalledApp, RootInfo, SearchResponse } from "../lib/types";

const loadDashboard = () => api.dashboard();
const loadInstalledApps = () => api.installedApps();

function AppColumn({
  title,
  apps,
  empty,
}: {
  title: string;
  apps: InstalledApp[];
  empty: string;
}) {
  return (
    <div>
      <h3 className="subheading">{title}</h3>
      {apps.length === 0 ? (
        <p className="muted">{empty}</p>
      ) : (
        <ul className="app-list">
          {apps.slice(0, 12).map((app) => (
            <li
              key={`${app.name}-${app.installLocation}`}
              className="app-item"
              title={app.installLocation || app.name}
            >
              <div className="app-item-head">
                <span className="app-name">{app.name}</span>
                <span className="app-size">{formatBytes(app.bytes)}</span>
              </div>
              <span className="muted app-evidence">
                {app.evidence} ·{" "}
                {app.sizeSource === "installer estimate" ? "estimated size" : "measured size"} ·
                confidence {Math.round(app.confidence * 100)}%
              </span>
            </li>
          ))}
          {apps.length > 12 && (
            <li className="muted app-evidence">…and {formatCount(apps.length - 12)} more</li>
          )}
        </ul>
      )}
    </div>
  );
}

export function DashboardPage() {
  const dashboard = useAsync(loadDashboard);
  const apps = useAsync(loadInstalledApps);
  const review = useReview();
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [searchQuery, setSearchQuery] = useState("");
  const [searchResult, setSearchResult] = useState<SearchResponse | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const [detailsCategory, setDetailsCategory] = useState<string | null>(null);
  const [detailsRetry, setDetailsRetry] = useState(0);
  const [details, setDetails] = useState<CategoryDetails | null>(null);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [detailsError, setDetailsError] = useState<string | null>(null);

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

  const runSearch = useCallback(
    async (event?: FormEvent<HTMLFormElement>) => {
      event?.preventDefault();
      const query = searchQuery.trim();
      if (!query) {
        setSearchResult(null);
        return;
      }
      setSearching(true);
      setSearchError(null);
      try {
        setSearchResult(await api.searchFiles(query, { limit: 8 }));
      } catch (error) {
        setSearchError(errorMessage(error));
      } finally {
        setSearching(false);
      }
    },
    [searchQuery],
  );

  useEffect(() => {
    if (!detailsCategory) {
      setDetails(null);
      return;
    }
    let cancelled = false;
    setDetailsLoading(true);
    setDetailsError(null);
    api
      .categoryDetails(detailsCategory, 12)
      .then((value) => {
        if (!cancelled) setDetails(value);
      })
      .catch((cause: unknown) => {
        if (!cancelled) setDetailsError(errorMessage(cause));
      })
      .finally(() => {
        if (!cancelled) setDetailsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [detailsCategory, detailsRetry]);

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

          <section className="card dashboard-search" aria-labelledby="dashboard-search-title">
            <div className="section-head">
              <h2 id="dashboard-search-title" className="section-title">
                Search your files
              </h2>
              <span className="muted">Looks only in the folders you chose</span>
            </div>
            <form
              className="search-bar"
              onSubmit={(event) => void runSearch(event)}
              role="search"
            >
              <input
                type="text"
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder="Search by name, folder, or text inside indexed documents…"
                aria-label="Search your files from the Dashboard"
              />
              <button type="submit" className="btn btn-primary" disabled={searching}>
                <Search size={14} aria-hidden="true" />
                Search
              </button>
            </form>

            {searchError && <ErrorNotice message={searchError} />}
            {searching && <LoadingNotice label="Searching your local index…" />}

            {searchResult && !searching && (
              <div className="search-results">
                <p className="muted">
                  {formatCount(searchResult.total)} {pluralize(searchResult.total, "match")} ·{" "}
                  {searchResult.tookMs} ms
                  {searchResult.mode === "full-text" ? " · searched inside documents" : ""}
                </p>
                {searchResult.hits.length === 0 ? (
                  <p className="muted">No files matched. Try a broader search.</p>
                ) : (
                  <ul className="category-summary-list">
                    {searchResult.hits.map((hit) => (
                      <li key={hit.file.id} className="category-summary-item">
                        <span title={hit.file.path}>
                          {hit.file.name}
                          <span className="muted"> · {categoryLabel(hit.file.category)}</span>
                        </span>
                        <span>{formatBytes(hit.file.size)}</span>
                        <span>{formatDate(hit.file.mtime)}</span>
                      </li>
                    ))}
                  </ul>
                )}
                <Link
                  className="btn dashboard-search-more"
                  to={`/my-files?q=${encodeURIComponent(searchResult.query)}`}
                >
                  View all results in My Files
                </Link>
              </div>
            )}
          </section>

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
              description="Add a folder on the Dashboard and run a scan. Nothing is scanned until you say so."
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
              <div className="section-head">
                <h2 id="categories-title" className="section-title">
                  What takes up space
                </h2>
                <span className="muted">Select a category to see what is inside it</span>
              </div>
              <ul className="category-summary-list">
                {[...data.byCategory]
                  .sort((a, b) => b.bytes - a.bytes)
                  .map((entry) => (
                    <li key={entry.category} className="category-summary-item">
                      <span>
                        <button
                          type="button"
                          className="category-link"
                          aria-expanded={detailsCategory === entry.category}
                          onClick={() =>
                            setDetailsCategory((current) =>
                              current === entry.category ? null : entry.category,
                            )
                          }
                        >
                          {categoryLabel(entry.category)}
                        </button>
                      </span>
                      <span>{formatBytes(entry.bytes)}</span>
                      <span>{formatCount(entry.count)} files</span>
                    </li>
                  ))}
              </ul>
            </section>
          )}

          {detailsCategory && (
            <section className="card" aria-labelledby="details-title">
              <div className="section-head">
                <h2 id="details-title" className="section-title">
                  Inside {categoryLabel(detailsCategory)}
                </h2>
                <button
                  type="button"
                  className="btn"
                  onClick={() => setDetailsCategory(null)}
                >
                  Close
                </button>
              </div>

              {detailsLoading && <LoadingNotice label="Grouping files in this category…" />}
              {detailsError && (
                <ErrorNotice
                  message={detailsError}
                  onRetry={() => setDetailsRetry((value) => value + 1)}
                />
              )}

              {details && !detailsLoading && (
                <>
                  <p className="muted">
                    {formatBytes(details.totalBytes)} across {formatCount(details.fileCount)}{" "}
                    {pluralize(details.fileCount, "file")}
                  </p>
                  <div className="details-grid">
                    <div>
                      <h3 className="subheading">By extension</h3>
                      {details.byExtension.length === 0 ? (
                        <p className="muted">No files in this category.</p>
                      ) : (
                        <ul className="category-summary-list">
                          {details.byExtension.map((entry) => (
                            <li key={entry.ext} className="category-summary-item">
                              <span>{entry.ext ? `.${entry.ext}` : "(no extension)"}</span>
                              <span>{formatBytes(entry.bytes)}</span>
                              <span>{formatCount(entry.count)} files</span>
                            </li>
                          ))}
                        </ul>
                      )}
                    </div>
                    <div>
                      <h3 className="subheading">By folder</h3>
                      {details.byFolder.length === 0 ? (
                        <p className="muted">No folders in this category.</p>
                      ) : (
                        <ul className="category-summary-list">
                          {details.byFolder.map((entry) => (
                            <li key={entry.path} className="category-summary-item">
                              <span title={entry.path}>{entry.path || "(root)"}</span>
                              <span>{formatBytes(entry.bytes)}</span>
                              <span>{formatCount(entry.count)} files</span>
                            </li>
                          ))}
                        </ul>
                      )}
                    </div>
                  </div>

                  <h3 className="subheading">Largest files</h3>
                  {details.largestFiles.length === 0 ? (
                    <p className="muted">No files to show.</p>
                  ) : (
                    <div className="media-table">
                      <FileTable
                        files={details.largestFiles}
                        caption={`Largest ${categoryLabel(detailsCategory)} files`}
                      />
                    </div>
                  )}
                </>
              )}
            </section>
          )}

          <section className="card" aria-labelledby="apps-title">
            <div className="section-head">
              <h2 id="apps-title" className="section-title">
                Applications
              </h2>
              <button type="button" className="btn" onClick={apps.reload}>
                Refresh
              </button>
            </div>

            {apps.loading && !apps.data && (
              <LoadingNotice label="Reading your installed applications…" />
            )}
            {apps.error && <ErrorNotice message={apps.error} onRetry={apps.reload} />}

            {apps.data && (
              <>
                <div className="stats-grid app-stats">
                  <StatCard
                    title="Installed by you"
                    value={formatCount(apps.data.userCount)}
                    subtitleOf={`${formatBytes(apps.data.userBytes)} across non-system apps`}
                  />
                  <StatCard
                    title="System apps"
                    value={formatCount(apps.data.systemCount)}
                    subtitleOf={`${formatBytes(apps.data.systemBytes)} marked by your system`}
                  />
                </div>
                <div className="app-columns">
                  <AppColumn
                    title="Non-system applications"
                    apps={apps.data.userApps}
                    empty="No non-system applications were found."
                  />
                  <AppColumn
                    title="System apps"
                    apps={apps.data.systemApps}
                    empty="No system applications were found."
                  />
                </div>
                <ul className="notes-list">
                  {apps.data.notes.map((note) => (
                    <li key={note}>{note}</li>
                  ))}
                </ul>
              </>
            )}
          </section>

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
