import { EmptyState } from "../components/EmptyState";

export function CategorisationPage() {
  return (
    <div className="page">
      <h1 className="page-title">Automatic Categorisation</h1>
      <p className="page-subtitle">AI-powered file type classification</p>

      <div className="categorisation-interface">
        <div className="category-cards">
          <div className="category-card">
            <div className="category-icon">📄</div>
            <div className="category-info">
              <h3>Documents</h3>
              <p>PDFs, Word docs, spreadsheets</p>
            </div>
          </div>

          <div className="category-card">
            <div className="category-icon">🖼️</div>
            <div className="category-info">
              <h3>Images</h3>
              <p>Photos, screenshots, graphics</p>
            </div>
          </div>

          <div className="category-card">
            <div className="category-icon">📹</div>
            <div className="category-info">
              <h3>Videos</h3>
              <p>Movies, recordings, presentations</p>
            </div>
          </div>
        </div>

        <div className="action-controls">
          <button className="btn">
            Auto-Categorise Selected Files
          </button>
          <button className="btn btn-primary">
            Scan Entire Drive
          </button>
        </div>
      </div>

      <div className="categorisation-stats">
        <div className="stat-item">
          <div className="stat-number">85%</div>
          <div className="stat-label">Auto-categorised</div>
        </div>
        <div className="stat-item">
          <div className="stat-number">12</div>
          <div className="stat-label">Uncategorised files</div>
        </div>
      </div>

      <EmptyState
        title="No files categorised yet"
        description="Select files or a drive to begin auto-categorisation."
        icon="file-text"
        buttonText="Start Categorisation"
      />
    </div>
  );
}