import { Search, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { EmptyState } from "../components/EmptyState";
import { FileTable } from "../components/FileTable";
import { ErrorNotice, LoadingNotice, NoResults, SuccessNotice } from "../components/Notice";
import { api, errorMessage } from "../lib/api";
import { categoryLabel, formatBytes, formatCount, pluralize } from "../lib/format";
import { bumpData, useAsync, useDebouncedValue } from "../lib/hooks";
import { useReview } from "../lib/review";
import type { FileEntry, SearchFilters } from "../lib/types";

const PAGE_SIZE = 100;

const CATEGORY_OPTIONS = [
  "documents",
  "images",
  "video",
  "audio",
  "archives",
  "installers",
  "other",
];

interface BrowseResult {
  files: FileEntry[];
  total: number;
  interpretation: string[];
  mode: string;
  tookMs: number;
}

export function MyFilesPage() {
  const review = useReview();
  const [searchParams] = useSearchParams();
  const [query, setQuery] = useState(() => searchParams.get("q") ?? "");
  const debouncedQuery = useDebouncedValue(query, 300);
  const [category, setCategory] = useState("");
  const [sort, setSort] = useState("mtime_desc");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    setOffset(0);
    setSelected(new Set());
  }, [debouncedQuery, category, sort]);

  const loader = useCallback(async (): Promise<BrowseResult> => {
    const filters: SearchFilters = {
      categories: category ? [category] : [],
      sort,
      limit: PAGE_SIZE,
    };

    if (debouncedQuery.trim()) {
      const response = await api.searchFiles(debouncedQuery.trim(), filters);
      return {
        files: response.hits.map((hit) => ({ ...hit.file, snippet: hit.snippet })),
        total: response.total,
        interpretation: response.interpretation,
        mode: response.mode,
        tookMs: response.tookMs,
      };
    }

    const page = await api.listFiles(filters, PAGE_SIZE, offset);
    return {
      files: page.items,
      total: page.total,
      interpretation: [],
      mode: "browse",
      tookMs: 0,
    };
  }, [debouncedQuery, category, sort, offset]);

  const browse = useAsync(loader);

  const files = useMemo(() => browse.data?.files ?? [], [browse.data]);
  const selectedFiles = useMemo(
    () => files.filter((file) => selected.has(file.id)),
    [files, selected],
  );
  const selectedBytes = selectedFiles.reduce((sum, file) => sum + file.size, 0);

  function toggle(id: number) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleAll(checked: boolean) {
    setSelected((current) => {
      const next = new Set(current);
      for (const file of files) {
        if (checked) next.add(file.id);
        else next.delete(file.id);
      }
      return next;
    });
  }

  async function quarantineSelected() {
    const ids = selectedFiles.map((file) => file.id);
    if (ids.length === 0) return;

    const confirmed = await review({
      title: `Move ${ids.length} ${pluralize(ids.length, "file")} to quarantine?`,
      message:
        "The files are moved into an app-managed holding area. You can put them back exactly where they were from the Cleanup Centre.",
      details: [
        { label: "Files", value: formatCount(ids.length) },
        { label: "Total size", value: formatBytes(selectedBytes) },
        {
          label: "Destination",
          value: "Digital Mess Cleaner quarantine",
        },
      ],
      note: "Reversible. Nothing is permanently deleted in this step.",
      primaryText: "Move to quarantine",
      secondaryText: "Keep files",
      tone: "caution",
    });
    if (!confirmed) return;

    setActionError(null);
    setNotice(null);
    setBusy(true);
    try {
      const result = await api.quarantineFiles(ids);
      bumpData();
      setSelected(new Set());
      setNotice(
        `Moved ${formatCount(result.moved)} ${pluralize(result.moved, "file")} to quarantine (${formatBytes(result.totalBytes)}).${
          result.failed > 0 ? ` ${result.failed} could not be moved.` : ""
        }`,
      );
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const total = browse.data?.total ?? 0;
  const hasPrev = offset > 0;
  const hasNext = offset + PAGE_SIZE < total;

  return (
    <div className="page">
      <div className="page-heading">
        <div>
          <h1 className="page-title">My Files</h1>
          <p className="page-subtitle">Browse and manage the files you have indexed</p>
        </div>
      </div>

      <div className="search-interface">
        <form
          className="search-bar"
          onSubmit={(event) => {
            event.preventDefault();
            browse.reload();
          }}
          role="search"
        >
          <input
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search by name, folder, or text inside indexed documents…"
            aria-label="Search your files"
          />
          <button type="submit" className="btn btn-primary">
            <Search size={14} aria-hidden="true" />
            Search
          </button>
        </form>

        <div className="filter-row">
          <label className="filter-field">
            <span>Category</span>
            <select value={category} onChange={(event) => setCategory(event.target.value)}>
              <option value="">All categories</option>
              {CATEGORY_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {categoryLabel(option)}
                </option>
              ))}
            </select>
          </label>

          <label className="filter-field">
            <span>Sort</span>
            <select value={sort} onChange={(event) => setSort(event.target.value)}>
              <option value="mtime_desc">Newest first</option>
              <option value="mtime_asc">Oldest first</option>
              <option value="size_desc">Largest first</option>
              <option value="name_asc">Name A–Z</option>
              <option value="name_desc">Name Z–A</option>
            </select>
          </label>

          {browse.data && (
            <span className="filter-meta">
              {formatCount(total)} {pluralize(total, "match")}
              {browse.data.mode === "full-text" ? " · searched inside documents" : ""}
              {browse.data.tookMs > 0 ? ` · ${browse.data.tookMs} ms` : ""}
            </span>
          )}
        </div>

        {browse.data && browse.data.interpretation.length > 0 && (
          <ul className="interpretation" aria-label="How your search was understood">
            {browse.data.interpretation.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        )}
      </div>

      {actionError && <ErrorNotice message={actionError} />}
      {notice && <SuccessNotice message={notice} />}

      {selectedFiles.length > 0 && (
        <div className="selection-bar" role="region" aria-label="Selected files">
          <span>
            {formatCount(selectedFiles.length)}{" "}
            {pluralize(selectedFiles.length, "file")} selected · {formatBytes(selectedBytes)}
          </span>
          <div className="selection-actions">
            <button
              type="button"
              className="btn btn-caution"
              onClick={() => void quarantineSelected()}
              disabled={busy}
            >
              <ShieldCheck size={14} aria-hidden="true" />
              Move to quarantine
            </button>
            <button type="button" className="btn" onClick={() => setSelected(new Set())}>
              Clear selection
            </button>
          </div>
        </div>
      )}

      {browse.loading && !browse.data && <LoadingNotice label="Reading your local index…" />}

      {browse.error && (
        <ErrorNotice
          message={browse.error}
          onRetry={browse.reload}
          desktopOnly={browse.errorCode === "not_desktop"}
        />
      )}

      {!browse.loading && !browse.error && files.length === 0 && (
        <>
          {query.trim() || category ? (
            <NoResults message="No files match those filters. Try a broader search or a different category." />
          ) : (
            <EmptyState
              title="No files indexed yet"
              description="Add a folder on the Dashboard and run a scan. Only folders you choose are read."
              icon="folder"
            />
          )}
        </>
      )}

      {files.length > 0 && (
        <>
          <FileTable
            files={files}
            selected={selected}
            onToggle={toggle}
            onToggleAll={toggleAll}
            caption="Your indexed files"
          />

          {(hasPrev || hasNext) && (
            <div className="pager">
              <button
                type="button"
                className="btn"
                onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
                disabled={!hasPrev}
              >
                Previous
              </button>
              <span className="pager-label">
                {formatCount(offset + 1)}–{formatCount(Math.min(offset + PAGE_SIZE, total))} of{" "}
                {formatCount(total)}
              </span>
              <button
                type="button"
                className="btn"
                onClick={() => setOffset(offset + PAGE_SIZE)}
                disabled={!hasNext}
              >
                Next
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
