import { EmptyState } from "../components/EmptyState";

export function SearchPage() {
  return (
    <div className="page">
      <h1 className="page-title">Full-Text Search</h1>
      <p className="page-subtitle">Search across file content and metadata</p>

      <div className="search-interface">
        <div className="search-bar">
          <input
            type="text"
            placeholder="Search files and content..."
            aria-label="Search"
            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
          />
          <button className="btn btn-primary" onClick={handleSearch}>
            Search
          </button>
        </div>
      </div>

      <div className="search-results">
        <h3>Results</h3>
        <div className="no-results">
          <p>Enter a search term to find files</p>
        </div>
      </div>

      <EmptyState
        title="No results found"
        description="Try different search terms or metadata filters."
        icon="search"
      />
    </div>
  );
}

function handleSearch() {
  // Trigger search - will be integrated with backend
}