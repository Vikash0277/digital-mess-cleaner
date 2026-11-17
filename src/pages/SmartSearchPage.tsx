import { Search, Sparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { FileTable } from "../components/FileTable";
import { ErrorNotice, LoadingNotice, NoResults } from "../components/Notice";
import { api } from "../lib/api";
import { formatCount, pluralize } from "../lib/format";
import { useAsync } from "../lib/hooks";
import type { SearchFilters, SearchResponse } from "../lib/types";

const RECENT_KEY = "dmc.recentSearches";

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
  const [sort, setSort] = useState("mtime_desc");
  const [recent, setRecent] = useState<string[]>(() => readRecent());

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
    const filters: SearchFilters = { sort, limit: 200 };
    return api.searchFiles(query, filters);
  }, [submitted, sort]);

  const search = useAsync(loader);

  function runSearch(value: string) {
    const query = value.trim();
    if (!query) return;
    setDraft(query);
    setSubmitted(query);
    setRecent((current) => [query, ...current.filter((item) => item !== query)].slice(0, 6));
  }

  const response = search.data;

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
                <select value={sort} onChange={(event) => setSort(event.target.value)}>
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
                ? " — names and folders only. Run content analysis to search inside documents."
                : " — text inside indexed documents is searchable."}
            </p>
          </section>

          <FileTable
            files={response.hits.map((hit) => ({ ...hit.file, snippet: hit.snippet }))}
            caption={`Results for ${response.query}`}
          />
        </>
      )}
    </div>
  );
}
