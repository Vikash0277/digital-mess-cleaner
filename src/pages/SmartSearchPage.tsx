import { Search, Sparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { FileTable } from "../components/FileTable";
import { ErrorNotice, LoadingNotice, NoResults } from "../components/Notice";
import { ProgressBar } from "../components/ProgressBar";
import { api, errorMessage } from "../lib/api";
import { formatCount, pluralize } from "../lib/format";
import { useAsync } from "../lib/hooks";
import type {
  SearchFilters,
  SearchResponse,
  TextIndexProgress,
  TextIndexReport,
  TextIndexStatus,
} from "../lib/types";

const RECENT_KEY = "dmc.recentSearches";
const PAGE_SIZE = 100;

function readRecent(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((item): item is string => typeof item === "string").slice(0, 6);
  } catch {
    return [];
  }
}

export function SmartSearchPage() {
  const [draft, setDraft] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [sort, setSort] = useState("relevance");
  const [offset, setOffset] = useState(0);
  const [recent, setRecent] = useState<string[]>(() => readRecent());
  const [status, setStatus] = useState<TextIndexStatus | null>(null);
  const [ftsHealthy, setFtsHealthy] = useState<boolean | null>(null);
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<TextIndexProgress | null>(null);
  const [report, setReport] = useState<TextIndexReport | null>(null);
  const [indexError, setIndexError] = useState<string | null>(null);
  const [indexBusy, setIndexBusy] = useState(false);

  const loadStatus = useCallback(async () => {
    try {
      const [nextStatus, healthy] = await Promise.all([api.textIndexStatus(), api.ftsCheck()]);
      setStatus(nextStatus);
      setFtsHealthy(healthy);
    } catch (cause) {
      setIndexError(errorMessage(cause));
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  useEffect(() => {
    let active = true;
    let cleanups: Array<() => void> = [];

    void import("@tauri-apps/api/event")
      .then(({ listen }) =>
        Promise.all([
          listen<TextIndexProgress>("textindex://progress", (event) => {
            if (active) setProgress(event.payload);
          }),
          listen<{ report?: TextIndexReport; error?: string }>("textindex://done", (event) => {
            if (!active) return;
            if (event.payload.error) setIndexError(event.payload.error);
            else if (event.payload.report) setReport(event.payload.report);
            setJobId(null);
            setProgress(null);
            setIndexBusy(false);
            void loadStatus();
          }),
        ]),
      )
      .then((listeners) => {
        if (active) cleanups = listeners;
        else listeners.forEach((unlisten) => unlisten());
      })
      .catch(() => {
        // Event listening is a convenience; the status still loads on mount.
      });

    return () => {
      active = false;
      cleanups.forEach((unlisten) => unlisten());
    };
  }, [loadStatus]);

  useEffect(() => {
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
    } catch {
      // Storage may be disabled; recent searches are a convenience only.
    }
  }, [recent]);

  const loader = useCallback(async (): Promise<SearchResponse | null> => {
    const query = submitted.trim();
    if (!query) return null;
    const filters: SearchFilters = { sort, limit: PAGE_SIZE, offset };
    return api.searchFiles(query, filters);
  }, [submitted, sort, offset]);

  const search = useAsync(loader);

  function runSearch(value: string) {
    const query = value.trim();
    if (!query) return;
    setDraft(query);
    setSubmitted(query);
    setOffset(0);
    setRecent((current) => [query, ...current.filter((item) => item !== query)].slice(0, 6));
  }

  async function startIndexing() {
    setIndexBusy(true);
    setIndexError(null);
    setReport(null);
    try {
      const id = await api.textIndexStart(2000);
      setJobId(id);
      setProgress(null);
    } catch (cause) {
      setIndexError(errorMessage(cause));
      setIndexBusy(false);
    }
  }

  async function stopIndexing() {
    if (!jobId) return;
    try {
      await api.textIndexCancel(jobId);
    } catch (cause) {
      setIndexError(errorMessage(cause));
    }
  }

  async function rebuildIndex() {
    setIndexBusy(true);
    setIndexError(null);
    try {
      await api.ftsRebuild();
      await loadStatus();
    } catch (cause) {
      setIndexError(errorMessage(cause));
    } finally {
      setIndexBusy(false);
    }
  }

  const response = search.data;
  const pageStart = offset + 1;
  const pageEnd = Math.min(offset + PAGE_SIZE, response?.total ?? 0);

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">Smart Search</h1>
          <p className="page-subtitle">
            Ask in plain language. Results come from your local index only.
          </p>
        </div>
      </div>

      <section className="card text-index-card" aria-labelledby="text-index-title">
        <div className="text-index-head flex flex-wrap items-start justify-between gap-2">
          <div>
            <h2 id="text-index-title" className="section-title">
              Text index coverage
            </h2>
            <p className="muted">
              Searchable text is read on this device. Indexing never changes your files.
            </p>
          </div>
          {status && (
            <span className="badge inline-flex items-center gap-2">
              {formatCount(status.indexed)} files searchable
            </span>
          )}
        </div>
        {status && (
          <dl className="text-index-numbers">
            <div>
              <dt>Text indexed</dt>
              <dd>{formatCount(status.indexed)}</dd>
            </div>
            <div>
              <dt>No readable text</dt>
              <dd>{formatCount(status.unsupported)}</dd>
            </div>
            <div>
              <dt>Not read yet</dt>
              <dd>{formatCount(status.none)}</dd>
            </div>
            <div>
              <dt>Missing on disk</dt>
              <dd>{formatCount(status.missing)}</dd>
            </div>
          </dl>
        )}
        {ftsHealthy === false && (
          <p className="search-mode">
            The local text index needs rebuilding. Rebuilding reads only your index and never
            changes your files.
          </p>
        )}
        {jobId && progress && (
          <div className="text-index-progress">
            <ProgressBar
              progress={progress.total > 0 ? (progress.done / progress.total) * 100 : 0}
              label={`Reading ${formatCount(progress.done)} of ${formatCount(progress.total)}`}
              showValue
            />
          </div>
        )}
        <div className="text-index-actions flex flex-wrap gap-2">
          {jobId ? (
            <button type="button" className="btn" onClick={() => void stopIndexing()}>
              Stop indexing
            </button>
          ) : (
            <button
              type="button"
              className="btn"
              onClick={() => void startIndexing()}
              disabled={indexBusy || !status || status.supportedRemaining === 0}
            >
              Index text from my files
            </button>
          )}
          {ftsHealthy === false && (
            <button
              type="button"
              className="btn"
              onClick={() => void rebuildIndex()}
              disabled={indexBusy || Boolean(jobId)}
            >
              Rebuild text index
            </button>
          )}
        </div>
        {report && (
          <p className="muted">
            Last run: {formatCount(report.indexed)} indexed, {formatCount(report.unsupported)} with
            no text
            {report.cancelled ? " (stopped early)" : ""}.
          </p>
        )}
        {indexError && (
          <p className="file-inspector-error" role="alert">
            {indexError}
          </p>
        )}
      </section>

      <form
        className="search-bar card search-hero"
        role="search"
        onSubmit={(event) => {
          event.preventDefault();
          runSearch(draft);
        }}
      >
        <Search size={18} aria-hidden="true" className="search-hero-icon" />
        <input
          type="text"
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="e.g. large PDFs from last year, invoices, screenshots"
          aria-label="Search your files"
        />
        <button type="submit" className="btn btn-primary">
          Search
        </button>
      </form>

      <div className="search-hints">
        <span className="search-hints-label">
          <Sparkles size={14} aria-hidden="true" /> Try
        </span>
        {[
          "large files over 1 GB",
          "documents from last year",
          "screenshots in 2024",
          "old videos",
        ].map((hint) => (
          <button
            key={hint}
            type="button"
            className="chip"
            onClick={() => runSearch(hint)}
          >
            {hint}
          </button>
        ))}
      </div>

      {recent.length > 0 && (
        <section className="recent-searches card" aria-labelledby="recent-searches-title">
          <h2 id="recent-searches-title" className="section-title">
            Recent searches
          </h2>
          <ul className="chip-list">
            {recent.map((item) => (
              <li key={item}>
                <button type="button" className="chip" onClick={() => runSearch(item)}>
                  {item}
                </button>
              </li>
            ))}
          </ul>
          <button type="button" className="btn" onClick={() => setRecent([])}>
            Clear recent searches
          </button>
        </section>
      )}

      {search.error && (
        <ErrorNotice
          message={search.error}
          onRetry={search.reload}
          desktopOnly={search.errorCode === "not_desktop"}
        />
      )}

      {search.loading && <LoadingNotice label="Searching your local index…" />}

      {response && !response.hits.length && (
        <NoResults
          message={`No files matched “${response.query}”. Try fewer words, or scan the folder that contains them.`}
        />
      )}

      {response && response.hits.length > 0 && (
        <>
          <section className="card search-summary" aria-label="Search summary">
            <div className="search-summary-head">
              <span>
                {formatCount(response.total)} {pluralize(response.total, "match")} in{" "}
                {response.tookMs} ms
              </span>
              <label className="filter-field">
                <span>Sort</span>
                <select
                  value={sort}
                  onChange={(event) => {
                    setSort(event.target.value);
                    setOffset(0);
                  }}
                >
                  <option value="relevance">Best match</option>
                  <option value="mtime_desc">Newest first</option>
                  <option value="mtime_asc">Oldest first</option>
                  <option value="size_desc">Largest first</option>
                  <option value="name_asc">Name A–Z</option>
                </select>
              </label>
            </div>

            {response.interpretation.length > 0 && (
              <>
                <p className="search-summary-label">How this was understood</p>
                <ul className="interpretation">
                  {response.interpretation.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </>
            )}

            <p className="search-mode">
              Search mode: <strong>{response.mode}</strong>
              {response.mode === "metadata"
                ? " — names and folders only. Index text to search inside documents."
                : " — text inside indexed documents is searchable."}
            </p>
          </section>

          <div className="search-pager font-ui text-ui-sm" aria-label="Search result pages">
            <button
              type="button"
              className="btn"
              disabled={offset === 0}
              onClick={() => setOffset((current) => Math.max(0, current - PAGE_SIZE))}
            >
              Previous
            </button>
            <span>
              Showing {formatCount(pageStart)}–{formatCount(pageEnd)} of{" "}
              {formatCount(response.total)}
            </span>
            <button
              type="button"
              className="btn"
              disabled={offset + PAGE_SIZE >= response.total}
              onClick={() => setOffset((current) => current + PAGE_SIZE)}
            >
              Next
            </button>
          </div>

          <FileTable
            files={response.hits.map((hit) => ({
              ...hit.file,
              snippet: hit.snippet,
              snippetParts: hit.snippetParts,
            }))}
            caption={`Results for ${response.query}`}
          />
        </>
      )}
    </div>
  );
}
